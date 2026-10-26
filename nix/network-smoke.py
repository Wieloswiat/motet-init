"""Two PDS accounts must deliver their current pilot profiles through relay and Tap.

A broken private-host transport, missed crawl, or wrong Tap collection filter
must fail this check rather than pass merely because each service is healthy.
"""

import base64
from datetime import datetime, timezone
import json
import os
import time
from urllib.error import HTTPError
from urllib.parse import urlsplit, urlunsplit
from urllib.request import Request, urlopen
from uuid import uuid4

from websockets.sync.client import connect


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
    try:
        return request(
            f"{pds}/xrpc/com.atproto.server.createAccount",
            {"handle": handle, "email": f"{handle}@example.test", "password": password},
        )
    except XrpcError as error:
        # Re-running a smoke may reuse its own accounts, never another failure.
        if error.status != 400 or error.body.get("error") != "HandleNotAvailable":
            raise
        return request(
            f"{pds}/xrpc/com.atproto.server.createSession",
            {"identifier": handle, "password": password},
        )


def main():
    relay = os.environ["MOTET_RELAY_URL"].rstrip("/")
    tap = os.environ["MOTET_TAP_URL"].rstrip("/")
    pds = os.environ["MOTET_PDS_RS_URL"].rstrip("/")
    tap_auth = basic(os.environ["MOTET_TAP_ADMIN_PASSWORD"])
    relay_auth = basic(os.environ["MOTET_RELAY_ADMIN_PASSWORD"])
    collection = "at.motet.pilot.profile"
    password = "motet-network-smoke-password"
    run_id = uuid4().hex
    tap_url = urlsplit(tap)
    channel = urlunsplit(
        ("wss" if tap_url.scheme == "https" else "ws", tap_url.netloc, "/channel", "", "")
    )

    # Subscribe before the writes: signal-collection enrollment is observed on
    # the firehose, not inserted into Tap's database or /repos/add by this test.
    with connect(channel, additional_headers={"Authorization": tap_auth}, open_timeout=30) as websocket:
        expected = {}
        for handle in ("alice.test", "bob.test"):
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
            expected[did] = (handle, result["cid"], record)
            print(f"Rust PDS: {handle} ({did}), profile {result['cid']}", flush=True)

        request(f"{relay}/admin/pds/requestCrawl", {"hostname": pds}, relay_auth)
        print(f"Relay: crawling {pds}", flush=True)

        deadline = time.monotonic() + 180
        while expected:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"Tap did not deliver profiles for {list(expected)}")
            try:
                event = json.loads(websocket.recv(timeout=remaining))
            except TimeoutError as error:
                raise TimeoutError(f"Tap did not deliver profiles for {list(expected)}") from error
            websocket.send(json.dumps({"type": "ack", "id": event["id"]}))
            if event["type"] != "record":
                continue
            update = event["record"]
            did = update["did"]
            if did not in expected or update["collection"] != collection or update["rkey"] != "self":
                continue
            handle, cid, record = expected[did]
            if update["action"] not in ("create", "update") or update.get("cid") != cid:
                continue
            if update.get("record") != record:
                raise AssertionError(f"Tap changed the profile for {did}: {update}")
            del expected[did]
            print(f"Tap: {handle} profile received and acknowledged ({cid})", flush=True)

    print("PASS: both account profiles traversed PLC identity, relay crawl and Tap's acknowledged channel.")


if __name__ == "__main__":
    main()
