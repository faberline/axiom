import hashlib
import hmac

import pytest
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from candidate import HandshakeError, public_bytes, session_key

ALICE = X25519PrivateKey.from_private_bytes(bytes(range(1, 33)))
BOB = X25519PrivateKey.from_private_bytes(bytes(range(101, 133)))
EVE = X25519PrivateKey.from_private_bytes(bytes(range(201, 233)))


def reference(private, peer, info):
    shared = private.exchange(peer.public_key())
    salt = b"".join(sorted((public_bytes(private), public_bytes(peer))))
    prk = hmac.new(salt, shared, hashlib.sha256).digest()
    return hmac.new(prk, info + b"\x01", hashlib.sha256).digest()


def test_both_sides_agree_on_the_reference_key():
    a = session_key(ALICE, public_bytes(BOB), context=b"chat")
    b = session_key(BOB, public_bytes(ALICE), context=b"chat")
    assert a == b
    assert len(a) == 32
    assert a == reference(ALICE, BOB, b"x25519-session/chat")


def test_context_and_peer_change_the_key():
    chat = session_key(ALICE, public_bytes(BOB), context=b"chat")
    assert session_key(ALICE, public_bytes(BOB), context=b"file") != chat
    assert session_key(ALICE, public_bytes(EVE), context=b"chat") != chat


def test_short_peer_key_is_a_handshake_error():
    with pytest.raises(HandshakeError, match="32 bytes"):
        session_key(ALICE, public_bytes(BOB)[:31], context=b"chat")


def test_low_order_peer_key_is_a_handshake_error():
    with pytest.raises(HandshakeError, match="low-order"):
        session_key(ALICE, bytes(32), context=b"chat")


def test_empty_context_is_rejected():
    assert issubclass(HandshakeError, ValueError)
    with pytest.raises(ValueError, match="context"):
        session_key(ALICE, public_bytes(BOB), context=b"")
