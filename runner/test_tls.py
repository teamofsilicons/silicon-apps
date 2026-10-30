"""Real local TLS handshakes; no package execution or production connections."""
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch

from server import Handler, RunnerHTTPServer, WorkerUnavailable, forward_to_worker, tls_context


@unittest.skipUnless(shutil.which("openssl"), "OpenSSL is required to generate disposable test certificates")
class TLSTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory()
        cls.root = Path(cls.directory.name).resolve()
        cls.cert, cls.key = cls.root / "server.crt", cls.root / "server.key"
        config = cls.root / "openssl.conf"
        config.write_text("[req]\nprompt=no\ndistinguished_name=subject\nx509_extensions=extensions\n[subject]\nCN=apps-native-worker-test\n[extensions]\nsubjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,digitalSignature,keyEncipherment,keyCertSign\nextendedKeyUsage=serverAuth\n")
        subprocess.run(["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-config", str(config), "-keyout", str(cls.key), "-out", str(cls.cert)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def server(self):
        context = tls_context({"APPS_RUNNER_TLS_CERT": str(self.cert), "APPS_RUNNER_TLS_KEY": str(self.key)})
        server = RunnerHTTPServer(("127.0.0.1", 0), Handler, context)
        server.token, server.images, server.workers = "test-credential-" + "x" * 32, {}, {}
        server.handshake_timeout = 0.15
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return server

    def test_partial_missing_or_invalid_tls_configuration_fails_closed(self):
        self.assertIsNone(tls_context({}))
        for environment in [
            {"APPS_RUNNER_TLS_CERT": str(self.cert)},
            {"APPS_RUNNER_TLS_KEY": str(self.key)},
            {"APPS_RUNNER_TLS_CERT": "relative.crt", "APPS_RUNNER_TLS_KEY": str(self.key)},
            {"APPS_RUNNER_TLS_CERT": str(self.cert), "APPS_RUNNER_TLS_KEY": str(self.root / "missing.key")},
        ]:
            with self.subTest(environment=environment), self.assertRaises(ValueError):
                tls_context(environment)
        with self.assertRaises(ssl.SSLError):
            tls_context({"APPS_RUNNER_TLS_CERT": str(self.cert), "APPS_RUNNER_TLS_KEY": str(self.cert)})

    def test_private_ca_authenticated_gateway_and_no_token_fallback(self):
        server = self.server()
        result = {"isolated": True, "target": "macos-aarch64", "validation": [
            {"command": command, "exit_code": 0, "stdout": "ok", "stderr": ""}
            for command in ("--help", "accounts --json", "login status --json")]}
        spec = {"url": f"https://127.0.0.1:{server.server_port}", "token": server.token}
        with patch.dict(os.environ, {"SSL_CERT_FILE": str(self.cert)}), patch("server.validate", return_value=result) as validate:
            self.assertEqual(forward_to_worker({"target": "macos-aarch64"}, spec), result)
            validate.assert_called_once()
            with self.assertRaisesRegex(WorkerUnavailable, "HTTP 401"):
                forward_to_worker({"target": "macos-aarch64"}, {**spec, "token": "wrong-token"})
            validate.assert_called_once()

    def test_untrusted_or_wrong_hostname_rejected_before_authenticated_request(self):
        server = self.server()
        for hostname, context in [("127.0.0.1", ssl.create_default_context()), ("localhost", ssl.create_default_context(cafile=str(self.cert)))]:
            connection = http.client.HTTPSConnection(hostname, server.server_port, timeout=2, context=context)
            with self.subTest(hostname=hostname), self.assertRaises(ssl.SSLCertVerificationError):
                connection.request("POST", "/validate", body="{}")
            connection.close()

    def test_idle_tls_connection_does_not_block_following_valid_client(self):
        server = self.server()
        idle = socket.create_connection(server.server_address, timeout=2)
        self.addCleanup(idle.close)
        connection = http.client.HTTPSConnection("127.0.0.1", server.server_port, timeout=2, context=ssl.create_default_context(cafile=str(self.cert)))
        self.addCleanup(connection.close)
        connection.request("POST", "/validate", body=json.dumps({}))
        self.assertEqual(connection.getresponse().status, 401)


if __name__ == "__main__":
    unittest.main()
