"""Two PDS profiles must reach the AppView through the relay and Tap.

A broken private-host transport, missed crawl, wrong Tap filter, or stalled
AppView consumer must fail instead of passing merely because services are healthy.
"""

import base64
from datetime import datetime, timezone
import json
import os
import time
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen
from uuid import uuid4


class XrpcError(RuntimeError):
    def __init__(self, url, status, body):
        self.status = status
        self.body = body
        super().__init__(f"{url}: HTTP {status}: {body}")


def request(url, body=None, authorization=None):
    headers = {"Content-Type": "application/json"}
    if authorization is not None:
        headers["Authorization"] = authorization
    data = None if body is None else json.dumps(body).encode()
    try:
        with urlopen(Request(url, data=data, headers=headers), timeout=30) as response:
            payload = response.read()
    except HTTPError as error:
        raise XrpcError(url, error.code, json.loads(error.read())) from error
    return json.loads(payload) if payload else None


def basic(password):
    return "Basic " + base64.b64encode(f"admin:{password}".encode()).decode()


def account(pds, handle, password):
    return request(
        f"{pds}/xrpc/com.atproto.server.createAccount",
        {"handle": handle, "email": f"{handle}@example.test", "password": password},
    )


def main():
    relay = os.environ["MOTET_RELAY_URL"].rstrip("/")
    pds = os.environ["MOTET_PDS_RS_URL"].rstrip("/")
    appview = os.environ["MOTET_APPVIEW_URL"].rstrip("/")
    relay_auth = basic(os.environ["MOTET_RELAY_ADMIN_PASSWORD"])
    collection = "at.motet.pilot.profile"
    password = "motet-network-smoke-password"
    run_id = uuid4().hex

    # Fresh accounts exercise automatic enrollment even after PLC restarts.
    expected = {}
    for name in ("alice", "bob"):
        handle = f"{name[0]}{run_id[:12]}.pds.motet.test"
        session = account(pds, handle, password)
        did = session["did"]
        record = {
            "$type": collection,
            "displayName": f"{handle} {run_id}",
            "createdAt": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        }
        result = request(
            f"{pds}/xrpc/com.atproto.repo.putRecord",
            {"repo": did, "collection": collection, "rkey": "self", "record": record, "validate": False},
            "Bearer " + session["accessJwt"],
        )
        expected[did] = (handle, result["uri"], result["cid"], record)
        print(f"Rust PDS: {handle} ({did}), profile {result['cid']}", flush=True)

    request(f"{relay}/admin/pds/requestCrawl", {"hostname": pds}, relay_auth)
    print(f"Relay: crawling {pds}", flush=True)

    # Tap consumers share events. Only the AppView may consume and acknowledge
    # them; the public exact-version query checks what it actually projected.
    deadline = time.monotonic() + 180
    while expected:
        for did, (handle, uri, cid, record) in list(expected.items()):
            query = urlencode({"uri": uri, "cid": cid})
            try:
                version = request(f"{appview}/xrpc/at.motet.pilot.getRecordVersion?{query}")
            except XrpcError as error:
                if error.status == 404 and error.body.get("error") == "NotFound":
                    continue
                raise
            if version["uri"] != uri or version["cid"] != cid or version["value"] != record:
                raise AssertionError(f"AppView returned the wrong profile version for {did}: {version}")
            del expected[did]
            print(f"AppView: {handle} profile projected ({cid})", flush=True)
        if expected:
            if time.monotonic() >= deadline:
                raise TimeoutError(f"AppView did not project profiles for {list(expected)}")
            time.sleep(1)

    print("PASS: both account profiles traversed PLC identity, relay, Tap and the AppView.")


if __name__ == "__main__":
    main()
