"""Routing tests for deploy/Caddyfile, run against a real Caddy in front of stub upstreams.

Set APPS_TEST_CADDY to a Caddy 2.11.7 binary for this machine to run them, for example
APPS_TEST_CADDY=/path/to/caddy python3 -m unittest discover -s deploy -p 'test_*.py'
Without it they are skipped. They use loopback ports only: no TLS, DNS or network access.
"""
import http.client
import http.server
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import threading
import time
import unittest

CADDY = os.environ.get('APPS_TEST_CADDY')
CADDYFILE = Path(__file__).with_name('Caddyfile')
SSE = re.compile(r'^/v1/(apps/[^/]+/)?events/stream$')
STRICT_CSP = "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
STORE_CSP = "default-src 'self'; script-src 'self' 'nonce-c3R1Yg==' 'strict-dynamic'"
PAD = 'x' * 4096  # Above Caddy's minimum length for compression.


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


class Upstream(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, role):
        self.role = role
        self.release_stream = threading.Event()
        super().__init__(('127.0.0.1', 0), Handler)


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *args):
        pass

    def reply(self, status, content_type, body, headers=()):
        data = body.encode()
        self.send_response(status)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(data)))
        self.send_header('X-Upstream', self.server.role)
        self.send_header('X-Seen-Forwarded-For', self.headers.get('X-Forwarded-For', ''))
        self.send_header('X-Seen-Host', self.headers.get('Host', ''))
        for name, value in headers:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(data)

    def stream(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Cache-Control', 'private, no-store, no-transform')
        self.send_header('X-Upstream', 'api')
        self.end_headers()  # No length: the stream stays open until the second event.
        self.wfile.write(b'event: first\ndata: 1\n\n')
        self.wfile.flush()
        self.server.release_stream.wait(10)
        self.wfile.write(b'event: second\ndata: 2\n\n')
        self.wfile.flush()
        self.close_connection = True

    def do_GET(self):
        path = self.path.split('?', 1)[0]
        if self.server.role == 'api':
            if SSE.match(path):
                return self.stream()
            cache = []
            if path.startswith('/v1'):
                cache = [('Cache-Control', 'private, no-store')]
            elif path != '/health':
                cache = [('Cache-Control', 'public, max-age=300')]
            return self.reply(200, 'application/json', json.dumps({'path': self.path, 'pad': PAD}), cache)
        page = '<!doctype html><html><body><main>Silicon Apps store %s</main><p>%s</p></body></html>' % (self.path, PAD)
        self.reply(200, 'text/html; charset=utf-8', page, [('Content-Security-Policy', STORE_CSP)])

    def do_POST(self):
        size = int(self.headers.get('Content-Length') or 0)
        received = 0
        while received < size:
            chunk = self.rfile.read(min(65536, size - received))
            if not chunk:
                break
            received += len(chunk)
        self.reply(200, 'application/json', json.dumps({'received': received}))


@unittest.skipUnless(CADDY, 'Set APPS_TEST_CADDY to a Caddy 2.11.7 binary to run the routing tests.')
class CaddyRoutingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        root = Path(cls.temporary.name)
        cls.release = root / 'release'
        (cls.release / 'store/.next/static/chunks').mkdir(parents=True)
        (cls.release / 'web').mkdir()
        cls.chunk = b'console.log("hashed chunk");\n' * 64
        (cls.release / 'store/.next/static/chunks/app-0123abcd.js').write_bytes(cls.chunk)
        cls.installers = {name: ('# installer %s\n' % name).encode() * 40 for name in ('install.sh', 'install.ps1')}
        for name, data in cls.installers.items():
            (cls.release / 'web' / name).write_bytes(data)
        cls.api, cls.store = Upstream('api'), Upstream('store')
        for server in (cls.api, cls.store):
            threading.Thread(target=server.serve_forever, daemon=True).start()
        cls.port = free_port()
        source = CADDYFILE.read_text()
        for site in ('apps.teamofsilicons.com', 'developer.teamofsilicons.com'):
            source = source.replace('\n%s {' % site, '\nhttp://%s:%d {' % (site, cls.port))
        source = (source.replace('127.0.0.1:4310', '127.0.0.1:%d' % cls.api.server_port)
                  .replace('127.0.0.1:4320', '127.0.0.1:%d' % cls.store.server_port)
                  .replace('/opt/silicon-apps/current', str(cls.release)))
        config = root / 'Caddyfile'
        config.write_text('{\n\tadmin off\n\tpersist_config off\n\tauto_https off\n\tdefault_bind 127.0.0.1\n}\n' + source)
        home = root / 'home'
        env = dict(os.environ, HOME=str(home), XDG_DATA_HOME=str(home / 'data'), XDG_CONFIG_HOME=str(home / 'config'))
        cls.log = (root / 'caddy.log').open('wb')
        cls.caddy = subprocess.Popen([CADDY, 'run', '--config', str(config), '--adapter', 'caddyfile'],
                                     stdout=cls.log, stderr=subprocess.STDOUT, env=env)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if cls.caddy.poll() is not None:
                break
            try:
                socket.create_connection(('127.0.0.1', cls.port), timeout=1).close()
                return
            except OSError:
                time.sleep(0.1)
        cls.tearDownClass()
        raise RuntimeError('Caddy did not start; see its log output.')

    @classmethod
    def tearDownClass(cls):
        cls.api.release_stream.set()
        if cls.caddy.poll() is None:
            cls.caddy.terminate()
            try:
                cls.caddy.wait(10)
            except subprocess.TimeoutExpired:
                cls.caddy.kill()
                cls.caddy.wait()
        cls.log.close()
        for server in (cls.api, cls.store):
            server.shutdown()
            server.server_close()
        cls.temporary.cleanup()

    def request(self, path, host='apps.teamofsilicons.com', method='GET', body=None, headers=None):
        connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=15)
        try:
            connection.request(method, path, body=body, headers={'Host': host, **(headers or {})})
            response = connection.getresponse()
            return response, response.read()
        finally:
            connection.close()

    def assert_security_headers(self, response):
        self.assertEqual(response.getheader('X-Content-Type-Options'), 'nosniff')
        self.assertEqual(response.getheader('X-Frame-Options'), 'DENY')
        self.assertEqual(response.getheader('Referrer-Policy'), 'strict-origin-when-cross-origin')
        self.assertEqual(response.getheader('Strict-Transport-Security'), 'max-age=31536000')
        self.assertIsNone(response.getheader('Server'))

    def test_api_and_discovery_documents_reach_the_api_with_a_strict_policy(self):
        for path, cache in [('/v1', 'private, no-store'), ('/v1/apps?q=notes', 'private, no-store'),
                            ('/v1/events', 'private, no-store'), ('/v1/apps/ring/events', 'private, no-store'),
                            ('/health', 'no-store'), ('/openapi.json', 'public, max-age=300'),
                            ('/.well-known/agent.json', 'public, max-age=300'),
                            ('/.well-known/agent-card.json', 'public, max-age=300'),
                            ('/.well-known/silicon-apps-keys.json', 'public, max-age=300')]:
            with self.subTest(path=path):
                response, body = self.request(path)
                self.assertEqual(response.status, 200)
                self.assertEqual(response.getheader('X-Upstream'), 'api')
                self.assertEqual(json.loads(body)['path'], path)
                self.assertEqual(response.getheader('Cache-Control'), cache)
                self.assertEqual(response.getheader('Content-Security-Policy'), STRICT_CSP)
                self.assert_security_headers(response)

    def test_every_other_path_is_the_store_and_keeps_its_own_policy(self):
        for path in ('/', '/search?q=notes', '/apps/ring', '/authors/0b5f', '/llms.txt', '/llms-full.txt',
                     '/robots.txt', '/sitemap.xml', '/mcp', '/.well-known/security.txt', '/store?q=x',
                     '/settings', '/favicon.ico', '/assets/index-old.js', '/v1x', '/healthz',
                     '/_next/image?url=%2Fog.png', '/_next/data/build/index.json'):
            with self.subTest(path=path):
                response, body = self.request(path, headers={'X-Forwarded-For': '203.0.113.9'})
                self.assertEqual(response.status, 200)
                self.assertEqual(response.getheader('X-Upstream'), 'store')
                self.assertIn(('<main>Silicon Apps store %s</main>' % path).encode(), body)
                self.assertEqual(response.msg.get_all('Content-Security-Policy'), [STORE_CSP])
                # A client cannot spoof its address: Caddy trusts no forwarded headers.
                self.assertEqual(response.getheader('X-Seen-Forwarded-For'), '127.0.0.1')
                self.assertEqual(response.getheader('X-Seen-Host'), 'apps.teamofsilicons.com')
                self.assert_security_headers(response)

    def test_hashed_static_files_are_immutable_and_never_fall_through_to_the_store(self):
        response, body = self.request('/_next/static/chunks/app-0123abcd.js')
        self.assertEqual(response.status, 200)
        self.assertEqual(body, self.chunk)
        self.assertIsNone(response.getheader('X-Upstream'))
        self.assertEqual(response.getheader('Cache-Control'), 'public, max-age=31536000, immutable')
        self.assertIn('javascript', response.getheader('Content-Type'))
        self.assert_security_headers(response)
        response, _ = self.request('/_next/static/chunks/missing.js')
        self.assertEqual(response.status, 404)
        self.assertIsNone(response.getheader('X-Upstream'))

    def test_installers_are_plain_text_revalidated_and_byte_identical(self):
        for name, data in self.installers.items():
            response, body = self.request('/' + name)
            self.assertEqual(response.status, 200)
            self.assertEqual(body, data)
            self.assertEqual(response.getheader('Content-Type'), 'text/plain; charset=utf-8')
            self.assertEqual(response.getheader('Cache-Control'), 'no-cache')
            self.assertEqual(response.getheader('Content-Security-Policy'), STRICT_CSP)
            self.assertIsNone(response.getheader('X-Upstream'))

    def test_event_streams_are_flushed_immediately_and_never_compressed(self):
        for path in ('/v1/events/stream', '/v1/apps/ring/events/stream?after=10'):
            with self.subTest(path=path):
                self.api.release_stream.clear()
                connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
                try:
                    connection.request('GET', path, headers={'Host': 'apps.teamofsilicons.com',
                                                             'Accept': 'text/event-stream',
                                                             'Accept-Encoding': 'gzip, zstd'})
                    response = connection.getresponse()
                    self.assertEqual(response.getheader('Content-Type'), 'text/event-stream')
                    self.assertIsNone(response.getheader('Content-Encoding'))
                    self.assertEqual(response.getheader('Content-Security-Policy'), STRICT_CSP)
                    # The first event arrives while the API is still holding the stream open.
                    first = b''
                    while not first.endswith(b'\n\n'):
                        first += response.read1(1024)
                    self.assertEqual(first, b'event: first\ndata: 1\n\n')
                    self.api.release_stream.set()
                    self.assertEqual(response.read(), b'event: second\ndata: 2\n\n')
                finally:
                    connection.close()

    def test_compression_applies_to_pages_and_api_responses(self):
        for path, upstream in (('/', 'store'), ('/v1/apps', 'api')):
            response, _ = self.request(path, headers={'Accept-Encoding': 'gzip'})
            self.assertEqual(response.getheader('X-Upstream'), upstream)
            self.assertEqual(response.getheader('Content-Encoding'), 'gzip')

    def test_body_limits(self):
        large = b'0' * (3 * 1000 * 1000)
        response, _ = self.request('/mcp', method='POST', body=large, headers={'Content-Type': 'application/json'})
        self.assertEqual(response.status, 413)
        response, body = self.request('/mcp', method='POST', body=b'{}', headers={'Content-Type': 'application/json'})
        self.assertEqual((response.status, response.getheader('X-Upstream')), (200, 'store'))
        response, body = self.request('/v1/apps/ring/packages', method='POST', body=large,
                                      headers={'Content-Type': 'application/octet-stream'})
        self.assertEqual((response.status, json.loads(body)['received']), (200, len(large)))

    def test_old_developer_and_docs_addresses_redirect(self):
        portal = 'https://developers.teamofsilicons.com'
        for host, path, location in [
            ('apps.teamofsilicons.com', '/developer', portal + '/'),
            ('apps.teamofsilicons.com', '/developer/apps/ring?tab=releases', portal + '/apps/ring/releases?tab=releases'),
            ('apps.teamofsilicons.com', '/developer/invitations', portal + '/invitations'),
            ('apps.teamofsilicons.com', '/docs', portal + '/docs/apps'),
            ('apps.teamofsilicons.com', '/docs/reference/manifest?source=legacy', portal + '/docs/apps/reference/manifest?source=legacy'),
            ('developer.teamofsilicons.com', '/developer/apps/ring', portal + '/apps/ring/publishing?'),
            ('developer.teamofsilicons.com', '/anything?x=1', portal + '/anything?x=1'),
        ]:
            with self.subTest(host=host, path=path):
                response, _ = self.request(path, host=host)
                self.assertEqual(response.status, 308)
                self.assertEqual(response.getheader('Location'), location)


class CaddyfileTextTests(unittest.TestCase):
    def test_upstreams_and_paths_match_the_installed_services(self):
        text = CADDYFILE.read_text()
        self.assertIn('reverse_proxy 127.0.0.1:4320', text)
        self.assertIn('root * /opt/silicon-apps/current/store/.next/static', text)
        self.assertIn('root * /opt/silicon-apps/current/web', text)
        self.assertIn('flush_interval -1', text)
        unit = Path(__file__).with_name('silicon-apps-store.service').read_text()
        self.assertIn('Environment=PORT=4320', unit)
        self.assertIn('Environment=HOSTNAME=127.0.0.1', unit)


if __name__ == '__main__':
    unittest.main()
