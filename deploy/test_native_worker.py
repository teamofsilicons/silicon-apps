"""Native worker configuration tests; never modify accounts, services or host files."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


launcher = load("native-worker-launch")


class NativeWorkerTests(unittest.TestCase):
    def config(self):
        root = Path(tempfile.gettempdir()).resolve() / "native-worker-test"
        return {"APPS_RUNNER_TOKEN": "x" * 40, "APPS_RUNNER_HOST": "10.0.0.10", "APPS_RUNNER_PORT": "4312", "APPS_RUNNER_TLS_CERT": str(root / "server.crt"), "APPS_RUNNER_TLS_KEY": str(root / "server.key"), "APPS_RUNNER_IMAGES": "{}", "TMPDIR": str(root / "jobs")}

    def test_launcher_discards_cloud_proxy_and_interactive_session_environment(self):
        inherited = {"PATH": "/usr/bin:/bin", "AWS_SECRET_ACCESS_KEY": "sentinel", "ACCOUNTS_TOKEN": "sentinel", "HTTPS_PROXY": "sentinel", "HOME": "/interactive/home", "SILICON_HOME": "/interactive/silicon", "SSL_CERT_FILE": "/interactive/trust.pem", "SystemRoot": "C:\\Windows"}
        result = launcher.environment(self.config(), inherited)
        for forbidden in ("AWS_SECRET_ACCESS_KEY", "ACCOUNTS_TOKEN", "HTTPS_PROXY", "SSL_CERT_FILE"):
            self.assertNotIn(forbidden, result)
        self.assertEqual(result["HOME"], self.config()["TMPDIR"])
        self.assertEqual(result["TEMP"], self.config()["TMPDIR"])
        self.assertEqual(result["SystemRoot"], "C:\\Windows")

    def test_launcher_requires_tls_and_rejects_extra_config_fields(self):
        for config in [{**self.config(), "APPS_RUNNER_TLS_KEY": ""}, {**self.config(), "TMPDIR": "relative"}, {**self.config(), "EXTRA": "unreviewed"}]:
            with self.subTest(config=config), self.assertRaises(ValueError):
                launcher.environment(config, {})

    @unittest.skipIf(os.name == "nt", "macOS service configuration imports POSIX account modules")
    def test_mac_launchd_runs_locked_account_and_recovers_without_credentials_in_plist(self):
        mac = load("native-worker-macos")
        config = mac.launch_configuration(Path("/opt/silicon-apps-native/venv/bin/python"))
        self.assertEqual(config["UserName"], "_siliconapps")
        self.assertTrue(config["KeepAlive"])
        self.assertTrue(config["RunAtLoad"])
        self.assertEqual(config["Umask"], 0o077)
        self.assertNotIn("APPS_RUNNER_TOKEN", str(config))
        self.assertNotIn("APPS_RUNNER_TLS_KEY", str(config))

    @unittest.skipIf(os.name == "nt", "POSIX runtime modes")
    def test_mac_runtime_normalizes_private_umask_without_modifying_symlink_target(self):
        mac = load("native-worker-macos")
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder) / "venv"
            root.mkdir(mode=0o700)
            library = root / "library.py"
            library.write_text("pass")
            library.chmod(0o600)
            binary = root / "executable"
            binary.write_text("script")
            binary.chmod(0o700)
            external = Path(folder) / "python"
            external.write_text("external runtime")
            external.chmod(0o700)
            (root / "python").symlink_to(external)
            with patch.object(mac.os, "chown") as chown:
                mac.readable_runtime(root, 451)
            self.assertEqual(root.stat().st_mode & 0o777, 0o750)
            self.assertEqual(library.stat().st_mode & 0o777, 0o640)
            self.assertEqual(binary.stat().st_mode & 0o777, 0o750)
            self.assertEqual(external.stat().st_mode & 0o777, 0o700)
            chown.assert_any_call(root / "python", 0, 451, follow_symlinks=False)


if __name__ == "__main__":
    unittest.main()
