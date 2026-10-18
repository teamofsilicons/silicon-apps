#!/bin/bash
# Bootstrap only a dedicated EC2 Mac. Never run this on a developer workstation.
set -euo pipefail
set +x
umask 077
export PATH=/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:/opt/homebrew/bin
export AWS_PAGER=""

usage() {
  cat <<'USAGE'
Usage: macos-bootstrap.sh --instance-id i-... --region REGION \
  --source-uri s3://BUCKET/releases/native-runners/SOURCE.tar.gz \
  --source-sha256 SHA256 --secret-arn ARN

Requires root on the named dedicated EC2 Mac, AWS CLI v2 from the AWS macOS AMI,
and its scoped instance role. The secret must contain TLS_CERT, TLS_KEY and
APPS_RUNNER_TOKEN. No secret values are accepted on the command line or printed.
USAGE
}

instance_id= region= source_uri= source_sha256= secret_arn=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --instance-id|--region|--source-uri|--source-sha256|--secret-arn)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      case "$1" in
        --instance-id) instance_id=$2 ;;
        --region) region=$2 ;;
        --source-uri) source_uri=$2 ;;
        --source-sha256) source_sha256=$2 ;;
        --secret-arn) secret_arn=$2 ;;
      esac
      shift 2 ;;
    --help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
done
[[ "$instance_id" =~ ^i-[a-f0-9]+$ ]] && [[ "$region" =~ ^[a-z]+-[a-z]+-[0-9]+$ ]] &&
  [[ "$source_sha256" =~ ^[a-f0-9]{64}$ ]] &&
  [[ "$source_uri" =~ ^s3://[a-z0-9.-]+/releases/native-runners/[a-zA-Z0-9./_-]+\.tar\.gz$ ]] &&
  [[ "$secret_arn" == "arn:aws:secretsmanager:$region:"*":secret:"* ]] || { usage >&2; exit 2; }
[ "$(/usr/bin/uname -s)" = Darwin ] && [ "$(/usr/bin/id -u)" = 0 ] || {
  printf '%s\n' 'Root on the named dedicated EC2 Mac is required.' >&2; exit 1;
}
command -v aws >/dev/null || { printf '%s\n' 'The AWS macOS AMI AWS CLI is required.' >&2; exit 1; }

bootstrap_dir=$(/usr/bin/mktemp -d /private/tmp/silicon-apps-macos-bootstrap.XXXXXX)
trap '/bin/rm -rf "$bootstrap_dir"' EXIT
metadata=http://169.254.169.254/latest
# IMDSv2 plus an exact instance ID prevents accidental workstation installation.
metadata_token=$(/usr/bin/curl --noproxy '*' --fail --silent --show-error --connect-timeout 2 --max-time 5 \
  -X PUT -H 'X-aws-ec2-metadata-token-ttl-seconds: 60' "$metadata/api/token")
printf 'header = "X-aws-ec2-metadata-token: %s"\n' "$metadata_token" > "$bootstrap_dir/imds.curl"
unset metadata_token
read_metadata() {
  /usr/bin/curl --noproxy '*' --fail --silent --show-error --connect-timeout 2 --max-time 5 \
    --config "$bootstrap_dir/imds.curl" "$metadata/meta-data/$1"
}
[ "$(read_metadata instance-id)" = "$instance_id" ] || { printf '%s\n' 'EC2 instance ID did not match.' >&2; exit 1; }
instance_type=$(read_metadata instance-type)
[[ "$instance_type" == mac*.metal ]] || { printf '%s\n' 'A dedicated EC2 Mac instance is required.' >&2; exit 1; }
private_ip=$(read_metadata local-ipv4)
/bin/rm -f "$bootstrap_dir/imds.curl"

python_version=3.14.8
python_sha256=507fc086c5c006ff875d344a75b4e67b8fb3c401f1bc4908c6250adb673d4907
python_pkg="$bootstrap_dir/python-$python_version-macos11.pkg"
/usr/bin/curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --tlsv1.2 \
  --connect-timeout 20 --max-time 600 \
  "https://www.python.org/ftp/python/$python_version/python-$python_version-macos11.pkg" --output "$python_pkg"
printf '%s  %s\n' "$python_sha256" "$python_pkg" | /usr/bin/shasum -a 256 --check --status
/usr/sbin/pkgutil --check-signature "$python_pkg" > "$bootstrap_dir/python-signature.txt"
# Pin the publisher of this exact release as well as its published SHA-256.
/usr/bin/grep -Fq 'Developer ID Installer: Python Software Foundation (BMM5U3QVKW)' "$bootstrap_dir/python-signature.txt"
/usr/bin/grep -Fq 'Notarization: trusted by the Apple notary service' "$bootstrap_dir/python-signature.txt"
/usr/sbin/installer -pkg "$python_pkg" -target / > "$bootstrap_dir/python-install.log" 2>&1 || {
  /bin/cat "$bootstrap_dir/python-install.log" >&2; exit 1;
}
python=/Library/Frameworks/Python.framework/Versions/3.14/bin/python3.14
[ "$("$python" -c 'import platform; print(platform.python_version())')" = "$python_version" ]

aws s3 cp "$source_uri" "$bootstrap_dir/source.tar.gz" --region "$region" --only-show-errors
printf '%s  %s\n' "$source_sha256" "$bootstrap_dir/source.tar.gz" | /usr/bin/shasum -a 256 --check --status
/bin/mkdir "$bootstrap_dir/source"
/usr/bin/tar -xzf "$bootstrap_dir/source.tar.gz" -C "$bootstrap_dir/source"
aws secretsmanager get-secret-value --secret-id "$secret_arn" --region "$region" --output json > "$bootstrap_dir/secret.json"
"$python" - "$bootstrap_dir" <<'PY'
import json
import os
from pathlib import Path
import sys

root = Path(sys.argv[1])
secret = json.loads(json.loads((root / "secret.json").read_text())["SecretString"])
for key, name in {"APPS_RUNNER_TOKEN": "token", "TLS_CERT": "worker.crt", "TLS_KEY": "worker.key"}.items():
    value = secret.get(key)
    if not isinstance(value, str) or not value.strip():
        raise SystemExit(f"Missing required worker secret field: {key}")
    path = root / name
    path.write_text(value)
    path.chmod(0o600)
(root / "secret.json").unlink()
PY
"$python" "$bootstrap_dir/source/deploy/native-worker-macos.py" \
  --source "$bootstrap_dir/source" --python "$python" --host "$private_ip" \
  --token-file "$bootstrap_dir/token" --tls-cert "$bootstrap_dir/worker.crt" --tls-key "$bootstrap_dir/worker.key"

"$python" - "$bootstrap_dir" "$source_sha256" "$source_uri" "$instance_id" "$instance_type" "$private_ip" "$python_sha256" <<'PY'
import datetime
import grp
import json
import os
from pathlib import Path
import platform
import shutil
import sys

root, source_sha256, source_uri, instance_id, instance_type, private_ip, python_sha256 = sys.argv[1:]
install = Path("/opt/silicon-apps-native")
acceptance = install / "acceptance"
acceptance.mkdir(exist_ok=True)
gid = grp.getgrnam("_siliconapps").gr_gid
os.chown(acceptance, 0, gid)
acceptance.chmod(0o750)
for source in (Path(root) / "source/runner").glob("test*.py"):
    target = acceptance / source.name
    shutil.copyfile(source, target)
    os.chown(target, 0, gid)
    target.chmod(0o640)
receipt = {"installed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
           "source_uri": source_uri, "source_sha256": source_sha256,
           "instance_id": instance_id, "instance_type": instance_type, "private_ip": private_ip,
           "os": platform.platform(), "machine": platform.machine(), "python": platform.python_version(),
           "python_package_sha256": python_sha256, "python_installer_publisher": "Python Software Foundation (BMM5U3QVKW)",
           "python_notarization_verified": True, "target_enabled": False,
           "acceptance": "Pending actual private TLS, sandbox, package and reboot checks."}
path = install / "bootstrap-receipt.json"
path.write_text(json.dumps(receipt, indent=2) + "\n")
os.chown(path, 0, gid)
path.chmod(0o640)
print(json.dumps(receipt))
PY
/bin/launchctl print system/com.teamofsilicons.apps-native-runner | /usr/bin/grep -E 'state =|pid =|last exit code ='
printf '%s\n' 'Bootstrap complete. Keep the target disabled until acceptance checks pass.'
