"""Backup traversal tests without AWS, service management or production data."""
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('apps_backup', Path(__file__).with_name('backup.py'))
backup = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(backup)


class BackupTests(unittest.TestCase):
    def test_disappearing_staging_files_are_skipped_and_committed_objects_remain(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            packages = root / 'packages'
            media = root / 'media/example'
            packages.mkdir()
            media.mkdir(parents=True)
            objects = {
                packages / ('a' * 64): b'complete package',
                media / ('b' * 64): b'complete image',
                media / ('b' * 64 + '.mime'): b'image/png',
            }
            for path, data in objects.items():
                path.write_bytes(data)
            for parent in (packages, media):
                (parent / '.upload.tmp').write_bytes(b'partial bytes')
                (parent / ('c' * 64 + '.old-upload.tmp')).write_bytes(b'old staging format')
            original = Path.iterdir

            def disappearing(path):
                children = list(original(path))
                for child in children:
                    if child.name.endswith('.tmp'):
                        child.unlink()
                return iter(children)

            output = io.BytesIO()
            with patch.object(Path, 'iterdir', disappearing), tarfile.open(fileobj=output, mode='w') as archive:
                backup.add_objects(archive, root)
            output.seek(0)
            with tarfile.open(fileobj=output, mode='r') as archive:
                self.assertEqual(set(archive.getnames()), {str(path.relative_to(root)) for path in objects})
                for path, data in objects.items():
                    self.assertEqual(archive.extractfile(str(path.relative_to(root))).read(), data)

    def test_committed_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'packages').mkdir()
            (root / 'packages' / ('a' * 64)).symlink_to(root / 'unrelated')
            with tarfile.open(fileobj=io.BytesIO(), mode='w') as archive:
                with self.assertRaisesRegex(ValueError, 'regular file'):
                    backup.add_objects(archive, root)


if __name__ == '__main__':
    unittest.main()
