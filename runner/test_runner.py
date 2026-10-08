import base64
import hashlib
import io
import json
import os
import platform
import tarfile
import tempfile
import unittest
from pathlib import Path

from server import execute, native_target, unpack, validate


def archive(files):
    result = io.BytesIO()
    with tarfile.open(fileobj=result, mode="w:gz") as tar:
        for path, data in files.items():
            entry = tarfile.TarInfo(path)
            entry.mode, entry.size = 0o755, len(data)
            tar.addfile(entry, io.BytesIO(data))
    return result.getvalue()


class RunnerTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "POSIX package mount permissions")
    def test_restrictive_service_umask_does_not_block_unprivileged_container_reads(self):
        manifest = {"schema_version": 1, "app_id": "sample", "version": "1.0.0", "command": "sample",
                    "targets": {"linux-x86_64": {"binary": "nested/bin/sample"}}}
        data = archive({"apps.yaml": json.dumps(manifest).encode(), "nested/bin/sample": b"executable"})
        previous_umask = os.umask(0o077)
        try:
            with tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                unpack(data, root, manifest)
                self.assertEqual((root / "nested").stat().st_mode & 0o777, 0o755)
                self.assertEqual((root / "nested/bin").stat().st_mode & 0o777, 0o755)
                self.assertEqual((root / "nested/bin/sample").stat().st_mode & 0o777, 0o755)
        finally:
            os.umask(previous_umask)

    def test_traversal_and_symlinks_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "Unsafe"):
                unpack(archive({"../escape": b"bad"}), Path(folder), {})
        result = io.BytesIO()
        with tarfile.open(fileobj=result, mode="w:gz") as tar:
            entry = tarfile.TarInfo("link")
            entry.type, entry.linkname = tarfile.SYMTYPE, "/etc/passwd"
            tar.addfile(entry)
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "Links"):
                unpack(result.getvalue(), Path(folder), {})

    @unittest.skipUnless(platform.system() == "Darwin", "native macOS sandbox")
    def test_required_commands_run_in_sandbox(self):
        manifest = {"schema_version": 1, "app_id": "sample", "version": "1.0.0", "command": "sample",
                    "targets": {native_target(): {"binary": "bin/sample"}}}
        script = b'''#!/bin/sh
case "$1" in
--help) echo 'Sample CLI help';;
accounts) echo '{"app_id":"sample"}';;
login) echo '{"authenticated":false}';;
*) exit 1;;
esac
'''
        data = archive({"apps.yaml": json.dumps(manifest).encode(), "bin/sample": script})
        result = validate({"app_id": "sample", "target": native_target(), "package_base64": base64.b64encode(data).decode(),
                           "package_sha256": hashlib.sha256(data).hexdigest(), "manifest": manifest}, {})
        self.assertTrue(result["isolated"])
        self.assertEqual(len(result["validation"]), 3)
        self.assertTrue(all(row["passed"] for row in result["validation"]), result)

    @unittest.skipUnless(platform.system() == "Darwin" and Path("/usr/bin/perl").is_file(), "native macOS fork policy")
    def test_native_process_fork_is_denied(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            probe = root / "fork-probe"
            probe.write_text("#!/usr/bin/perl\nmy $child = fork();\nif (defined($child)) { if ($child > 0) { kill 9, $child; waitpid($child, 0); } print \"unexpected fork\\n\"; exit 2; }\nprint \"fork denied\\n\"; exit 0;\n")
            probe.chmod(0o755)
            code, out, err = execute(root, "fork-probe", [], native_target(), {})
            self.assertEqual(code, 0, (out, err))
            self.assertEqual(out.strip(), "fork denied")

    @unittest.skipUnless(platform.system() == "Darwin", "native macOS sandbox")
    def test_host_read_write_and_network_denied(self):
        with tempfile.TemporaryDirectory() as outside, tempfile.TemporaryDirectory() as folder:
            sentinel = Path(outside).resolve() / "sentinel"
            sentinel.write_text("test content outside the sandbox")
            root = Path(folder).resolve()
            script = root / "probe"
            script.write_text(f'''#!/bin/sh
case "$1" in
read) exec /bin/cat "{sentinel}";;
write) echo bad > "{sentinel}";;
network) exec /usr/bin/curl --max-time 2 http://127.0.0.1:4310/health;;
esac
''')
            script.chmod(0o755)
            for operation in ["read", "write", "network"]:
                code, out, err = execute(root, "probe", [operation], native_target(), {})
                self.assertNotEqual(code, 0, (operation, out, err))
                self.assertNotIn("test content outside", out)
            self.assertEqual(sentinel.read_text(), "test content outside the sandbox")

class GatewayTests(unittest.TestCase):
    def request(self, target="windows-aarch64"):
        data = b"transport fixture, never executed"
        return {"app_id": "sample", "target": target, "package_base64": base64.b64encode(data).decode(),
                "package_sha256": hashlib.sha256(data).hexdigest(), "manifest": {}}

    def result(self, target="windows-aarch64"):
        return {"isolated": True, "target": target, "validation": [
            {"command": command, "exit_code": 0, "stdout": output, "stderr": "", "passed": True}
            for command, output in [("--help", "Help"), ("accounts --json", '{"app_id":"sample"}'), ("login status --json", '{"authenticated":false}')]]}

    def worker(self, result, status=200, headers=None):
        from http.server import HTTPServer, BaseHTTPRequestHandler
        import threading
        observed = []
        class FakeWorker(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass
            def do_POST(self):
                observed.append((self.path, self.headers.get("Authorization"), json.loads(self.rfile.read(int(self.headers["Content-Length"])))))
                body = json.dumps(result).encode()
                self.send_response(status)
                for name, value in (headers or {}).items():
                    self.send_header(name, value)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                try:
                    self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError):
                    pass
        server = HTTPServer(("127.0.0.1", 0), FakeWorker)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return {"url": f"http://127.0.0.1:{server.server_port}", "token": "worker-secret-" + "x" * 32}, observed

    def test_worker_config_rejects_insecure_remote_urls_credential_urls_and_short_tokens(self):
        from server import worker_config, image_config
        for url in ["http://worker.example", "https://user:password@worker.example", "https://worker.example/other", "https://worker.example?token=bad", "file:///tmp/worker"]:
            with self.assertRaises(ValueError):
                worker_config({"windows-aarch64": {"url": url, "token": "x" * 32}})
        with self.assertRaises(ValueError):
            worker_config({"windows-aarch64": {"url": "https://worker.example", "token": "short"}})
        with self.assertRaises(ValueError):
            image_config({"linux-x86_64": "debian:latest"})
        self.assertEqual(len(worker_config({"windows-aarch64": {"url": "https://worker.example", "token": "x" * 32}})), 1)

    def test_gateway_forwards_authenticated_exact_target_without_local_execution(self):
        from unittest.mock import patch
        spec, observed = self.worker(self.result())
        request = self.request()
        with patch("server.execute", side_effect=AssertionError("must not execute locally")):
            result = validate(request, {}, {"windows-aarch64": spec})
        self.assertTrue(result["isolated"])
        self.assertEqual(observed, [("/validate", "Bearer " + spec["token"], request)])

    def test_failed_worker_never_falls_back_and_wrong_target_is_rejected(self):
        from unittest.mock import patch
        from server import WorkerUnavailable
        for response, status in [(self.result(), 503), (self.result("linux-x86_64"), 200), ({"isolated": False}, 200)]:
            spec, _ = self.worker(response, status)
            with patch("server.execute", side_effect=AssertionError("must not execute locally")), self.assertRaises(WorkerUnavailable):
                validate(self.request(), {}, {"windows-aarch64": spec})

    def test_worker_errors_preserved_and_redirects_not_followed(self):
        from server import WorkerUnavailable, WorkerValidationError
        spec, _ = self.worker({"error": "exact executable error"}, 422)
        with self.assertRaises(WorkerValidationError) as raised:
            validate(self.request(), {}, {"windows-aarch64": spec})
        self.assertEqual(raised.exception.result, {"error": "exact executable error"})
        spec, _ = self.worker({}, 302, {"Location": "http://127.0.0.1:1/steal"})
        with self.assertRaises(WorkerUnavailable):
            validate(self.request(), {}, {"windows-aarch64": spec})

    def test_response_and_command_output_limits_are_enforced(self):
        from server import WorkerUnavailable, OUTPUT_BYTES
        result = self.result()
        result["validation"][0]["stdout"] = "x" * (OUTPUT_BYTES * 3 + 1)
        spec, _ = self.worker(result)
        with self.assertRaises(WorkerUnavailable):
            validate(self.request(), {}, {"windows-aarch64": spec})
        spec, _ = self.worker({"padding": "x" * (1024 * 1024 + 1)})
        with self.assertRaises(WorkerUnavailable):
            validate(self.request(), {}, {"windows-aarch64": spec})

    def test_windows_command_uses_hyperv_vm_readonly_mount_and_bounded_resources(self):
        from unittest.mock import patch
        image = "mcr.microsoft.com/windows/servercore@sha256:" + "a" * 64
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with patch("server.os.name", "nt"), patch("server.native_target", return_value=None), patch("server.bounded_run", return_value=(0, "ok", "")) as run, patch("server.subprocess.run") as cleanup:
                execute(root, "bin/sample.exe", ["--help"], "windows-aarch64", {"windows-aarch64": image})
            command = run.call_args.args[0]
            self.assertIn("--isolation=hyperv", command)
            self.assertIn("windows/arm64", command)
            self.assertIn("--network=none", command)
            self.assertIn("--cpu-count=1", command)
            self.assertIn("--memory=1g", command)
            self.assertIn("--user=ContainerUser", command)
            self.assertTrue(any("target=C:\\package,readonly" in arg for arg in command))
            cleanup.assert_called_once()

    def test_windows_device_paths_and_malformed_manifests_rejected(self):
        from server import regular_path
        for name in ["CON", "bin/AUX.txt", "trailing.", "bad ", "LPT1.exe"]:
            self.assertFalse(regular_path(name), name)
        with tempfile.TemporaryDirectory() as folder, self.assertRaisesRegex(ValueError, "targets"):
            unpack(archive({"apps.yaml": b"[]"}), Path(folder), {})


if __name__ == "__main__":
    unittest.main()
