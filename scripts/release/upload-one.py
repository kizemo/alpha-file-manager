import hashlib
import json
import os
import sys

import paramiko

CFG = r"E:\办公文件\L1网站\.vscode\sftp.json"
REMOTE_BASE = "/var/www/wordpress/alpha-file-manager"


def md5_of(path):
    h = hashlib.md5()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    if len(sys.argv) < 2:
        print("usage: upload_one.py <local-file>")
        return 2
    local = sys.argv[1]
    if not os.path.isfile(local):
        print("[ERR] local file missing:", local)
        return 2
    cfg = json.load(open(CFG, "r", encoding="utf-8"))
    for key in ("host", "port", "username", "password"):
        if key not in cfg:
            print("[ERR] sftp config missing field:", key)
            return 2
    remote = REMOTE_BASE + "/" + os.path.basename(local)
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(
        cfg["host"],
        port=int(cfg["port"]),
        username=cfg["username"],
        password=cfg["password"],
    )
    print("[upload] put", local, "->", remote, flush=True)
    sftp = client.open_sftp()
    sftp.put(local, remote)
    sftp.close()
    print("[upload] put done", flush=True)

    local_md5 = md5_of(local)
    local_size = os.path.getsize(local)
    stdin, stdout, stderr = client.exec_command(
        "md5sum '%s' && stat -c %%s '%s'" % (remote, remote)
    )
    out = stdout.read().decode("utf-8", "ignore").strip().splitlines()
    err = stderr.read().decode("utf-8", "ignore").strip()
    print("[verify] local  md5=%s size=%s" % (local_md5, local_size))
    print("[verify] remote", out)
    if err:
        print("[verify] stderr:", err)
    ok = len(out) >= 2 and out[0].split()[0] == local_md5 and int(out[1].strip()) == local_size
    print("REMOTE_MATCH" if ok else "REMOTE_MISMATCH")
    client.close()
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
