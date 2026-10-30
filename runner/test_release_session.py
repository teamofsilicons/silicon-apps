import base64
import hashlib
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import Mock
import urllib.error
import urllib.request

import server
from release_session import NotApproved, ReleaseHandler, ReleaseSession, read_token


class ReleaseSessionTests(unittest.TestCase):
    def setUp(self):
        self.data = b"approved release bytes, never executed by these tests"
        self.sha = hashlib.sha256(self.data).hexdigest()
        self.now = 100.0
        self.executor = Mock(return_value={"isolated": True, "target": "macos-aarch64", "validation": []})
        self.session = ReleaseSession("write", "macos-aarch64", self.sha, len(self.data), 3600,
                                      executor=self.executor, clock=lambda: self.now)
        self.request = {"app_id": "write", "target": "macos-aarch64", "package_sha256": self.sha,
                        "package_base64": base64.b64encode(self.data).decode(), "manifest": {"app_id": "write"}}

    def test_only_exact_approved_archive_reaches_existing_sandbox_validator(self):
        self.session.validate(self.request)
        self.executor.assert_called_once_with(self.request, {}, {})

    def test_unapproved_identity_target_hash_and_bytes_never_reach_unpack_or_execute(self):
        for changed in ({"app_id": "another-app"}, {"target": "macos-x86_64"}, {"package_sha256": "0" * 64},
                        {"package_base64": base64.b64encode(b"x" * len(self.data)).decode()},
                        {"package_base64": "!" * len(self.request["package_base64"])}, {"manifest": []},
                        {"extra_override": True}):
            with self.subTest(changed=changed), self.assertRaises(NotApproved):
                self.session.validate({**self.request, **changed})
        self.executor.assert_not_called()

    def test_lease_reserves_enough_time_for_bounded_validation_then_refuses_new_work(self):
        self.now += 3511
        with self.assertRaises(server.WorkerUnavailable):
            self.session.validate(self.request)
        self.executor.assert_not_called()
        with self.assertRaises(ValueError):
            ReleaseSession("write", "macos-aarch64", self.sha, len(self.data), 7201)

    @unittest.skipUnless(os.name == "posix", "Native Mac token files require POSIX permissions and O_NOFOLLOW")
    def test_private_token_file_rejects_public_permissions_and_symlinks(self):
        token = "test-secret-" + "a" * 40
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "token"
            path.write_text(token)
            path.chmod(0o600)
            self.assertEqual(read_token(path, {}), token)
            path.chmod(0o644)
            with self.assertRaises(ValueError):
                read_token(path, {})
            link = Path(folder) / "link"
            link.symlink_to(path)
            with self.assertRaises(OSError):
                read_token(link, {})

    def test_environment_token_without_command_line_secret(self):
        token = "test-secret-" + "a" * 40
        self.assertEqual(read_token(None, {"APPS_RELEASE_SESSION_TOKEN": token}), token)
        for invalid in ("", "a" * 31, "a" * 1025, "a" * 32 + " ", "a" * 32 + "\u00e9"):
            with self.subTest(token=invalid), self.assertRaises(ValueError):
                read_token(None, {"APPS_RELEASE_SESSION_TOKEN": invalid})
        with self.assertRaises(ValueError):
            read_token(None, {})

    def test_http_authentication_and_archive_gate_precede_validator(self):
        with server.RunnerHTTPServer(("127.0.0.1", 0), ReleaseHandler) as service:
            service.token, service.session = "test-session-token-" + "a" * 32, self.session
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            url = f"http://127.0.0.1:{service.server_address[1]}/validate"
            for token, body, status in (("wrong-token", self.request, 401),
                                        (service.token, {**self.request, "app_id": "another-app"}, 403),
                                        (service.token, self.request, 200)):
                thread = threading.Thread(target=service.handle_request)
                thread.start()
                request = urllib.request.Request(url, data=json.dumps(body).encode(),
                    headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
                try:
                    with opener.open(request, timeout=3) as response:
                        self.assertEqual(response.status, status)
                except urllib.error.HTTPError as error:
                    self.assertEqual(error.code, status)
                    error.close()
                finally:
                    thread.join(timeout=5)
                    self.assertFalse(thread.is_alive())
            self.executor.assert_called_once_with(self.request, {}, {})


if __name__ == "__main__":
    unittest.main()
