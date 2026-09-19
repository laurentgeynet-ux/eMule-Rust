//! Historical operation codes (opcodes) for eD2K, eMule extensions, and compressed payloads.

// ---------------------------------------------------------------------------
// Standard eD2K Opcodes (Protocol 0xE3)
// ---------------------------------------------------------------------------

/// Handshake initialization packet (`0x01`).
pub const OP_HELLO: u8 = 0x01;
/// Client login request to server (`0x01`).
pub const OP_LOGINREQUEST: u8 = 0x01;

/// Server status response with user and file counts (`0x34`, legacy `0x14`).
pub const OP_SERVERSTATUS: u8 = 0x34;
/// Legacy server status response opcode (`0x14`).
pub const OP_SERVERSTATUS_LEGACY: u8 = 0x14;

/// Server identification and metadata (`0x41`).
pub const OP_SERVERIDENT: u8 = 0x41;

/// Server message containing server greetings or notice text (`0x38`).
pub const OP_SERVERMESSAGE: u8 = 0x38;

/// Server assigns client ID and TCP flags (`0x40`).
pub const OP_IDCHANGE: u8 = 0x40;

/// Search request sent by client to server (`0x16`).
pub const OP_SEARCHREQUEST: u8 = 0x16;

/// Search result sent by server to client (`0x33`).
pub const OP_SEARCH_RESULT: u8 = 0x33;

/// Get sources request sent by client to server for a file (`0x19`).
pub const OP_GETSOURCES: u8 = 0x19;

/// Get sources request with obfuscation sent by client to server (`0x23`).
pub const OP_GETSOURCES_OBFU: u8 = 0x23;

/// Found sources response sent by server to client (`0x42`).
pub const OP_FOUNDSOURCES: u8 = 0x42;

/// Found sources response with obfuscation settings and optional user hashes (`0x44`).
pub const OP_FOUNDSOURCES_OBFU: u8 = 0x44;

/// Handshake response packet (`0x4C`).
pub const OP_HELLO_ANSWER: u8 = 0x4C;
/// Alias for OP_HELLO_ANSWER (`0x4C`).
pub const OP_HELLOANSWER: u8 = 0x4C;

/// Announce shared files to server (`0x15`).
pub const OP_OFFERFILES: u8 = 0x15;

/// Ask the server to relay a connection request to a LowID client (`0x1C`).
pub const OP_CALLBACKREQUEST: u8 = 0x1C;

/// Server-relayed callback notification to the LowID target (`0x35`).
pub const OP_CALLBACKREQUESTED: u8 = 0x35;

/// Server failed to relay a callback request (`0x36`).
pub const OP_CALLBACK_FAIL: u8 = 0x36;

/// Data block transfer packet (`0x46`).
pub const OP_SENDINGPART: u8 = 0x46;

/// Canonical eMule request for block ranges in a part (`0x47`).
pub const OP_REQUESTPARTS_EMULE: u8 = 0x47;

/// Legacy set requested file hash (`0x47`).
pub const OP_SETREQFILEID: u8 = 0x47;

/// Response indicating requested file is not shared (`0x48`).
pub const OP_FILEREQANSNOFILE: u8 = 0x48;

/// Request parts availability bitfield (`0x49`).
pub const OP_FILESTATUSREQ: u8 = 0x49;

/// Request specific block ranges in a part (`0x4B`).
pub const OP_REQUESTPARTS: u8 = 0x4B;

/// Notify completion of file download (`0x4D`).
pub const OP_END_OF_DOWNLOAD: u8 = 0x4D;

/// Canonical eMule set requested file hash / file status request (`0x4F`).
pub const OP_SETREQFILEID_EMULE: u8 = 0x4F;

/// Parts availability bitfield response (`0x50`).
pub const OP_FILESTATUS: u8 = 0x50;

/// Request hashset for multi-part file (`0x51`).
pub const OP_HASHSETREQUEST: u8 = 0x51;

/// Hashset payload response (`0x52`).
pub const OP_HASHSETANSWER: u8 = 0x52;

/// Request entry into upload queue (`0x54`).
pub const OP_STARTUPLOADREQ: u8 = 0x54;

/// Accept upload request and allocate slot (`0x55`).
pub const OP_ACCEPTUPLOADREQ: u8 = 0x55;

/// Cancel transfer or drop from queue (`0x56`).
pub const OP_CANCELTRANSFER: u8 = 0x56;

/// Signal client no longer needs parts (`0x57`).
pub const OP_OUTOFPARTS: u8 = 0x57;

/// Request filename by hash (`0x58`).
pub const OP_REQFILENAME: u8 = 0x58;

/// Filename response (`0x59`).
pub const OP_REQFILENAME_ANSWER: u8 = 0x59;

/// Queue ranking response in upload queue (`0x60`).
pub const OP_QUEUERANKING: u8 = 0x60;

/// Server list pushed by the connected server (`0x32`).
/// Payload: `u8 count` + `count` × (`u32 ip` + `u16 port`).
pub const OP_SERVERLIST: u8 = 0x32;

// ---------------------------------------------------------------------------
// Server UDP Opcodes (Protocol 0xE3 over UDP, eMule UDPSocket)
// ---------------------------------------------------------------------------

/// Global search request, extended tag form (`0x90`):
/// `<u32 tag_count><CTag CT_SERVER_UDPSEARCH_FLAGS>` + search expression.
pub const OP_GLOBSEARCHREQ3: u8 = 0x90;
/// Global search request (`0x92`): raw search expression, same payload as
/// `OP_SEARCHREQUEST` (eMule reuses the search packet verbatim).
pub const OP_GLOBSEARCHREQ2: u8 = 0x92;
/// Global sources request with file size (`0x94`): `hash16 + u32 size`.
pub const OP_GLOBGETSOURCES2: u8 = 0x94;
/// Global search request, legacy form (`0x98`): raw search expression.
pub const OP_GLOBSEARCHREQ: u8 = 0x98;
/// Server statistics request (`0x96`, empty payload).
pub const OP_GLOBSERVSTATREQ: u8 = 0x96;
/// Server statistics response (`0x97`): `u32 users + u32 files`.
pub const OP_GLOBSERVSTATRES: u8 = 0x97;
/// Global search result (`0x99`): search result file entries.
pub const OP_GLOBSEARCHRES: u8 = 0x99;
/// Global found sources response (`0x9B`): repeated
/// `hash16 + u8 count + count` × (`u32 ip` + `u16 port`).
pub const OP_GLOBFOUNDSOURCES: u8 = 0x9B;

// ---------------------------------------------------------------------------
// Client-to-Client UDP Opcodes (Protocol 0xC5, eMule ClientUDPSocket)
// ---------------------------------------------------------------------------

/// Keep-alive / re-request sent over UDP to a queued source (`0x90`).
/// Payload: `<HASH 16>` (+ optional extended info for UDP version > 2).
pub const OP_REASKFILEPING: u8 = 0x90;

/// UDP answer carrying the queue rank (`0x91`).
/// Payload: `[u16 part_status_count + bitfield]` (UDP ver > 3) + `u16 rank`.
pub const OP_REASKACK: u8 = 0x91;

/// UDP answer: the requested file is not shared (`0x92`, empty payload).
pub const OP_FILENOTFOUND: u8 = 0x92;

/// UDP answer: the remote upload queue is full (`0x93`, empty payload).
pub const OP_QUEUEFULL: u8 = 0x93;

/// UDP callback re-request relayed through a Kad buddy (`0x94`).
pub const OP_REASKCALLBACKUDP: u8 = 0x94;

// ---------------------------------------------------------------------------
// Extended eMule Opcodes (Protocol 0xC5)
// ---------------------------------------------------------------------------

/// Extended eMule handshake capabilities exchange (`0x01`).
pub const OP_EMULE_INFO: u8 = 0x01;
/// Canonical alias for `OP_EMULE_INFO`.
pub const OP_EMULEINFO: u8 = OP_EMULE_INFO;

/// Extended eMule handshake capabilities answer (`0x02`).
pub const OP_EMULE_INFO_ANSWER: u8 = 0x02;
/// Canonical alias for `OP_EMULE_INFO_ANSWER`.
pub const OP_EMULEINFOANSWER: u8 = OP_EMULE_INFO_ANSWER;

/// Request AICH root hash (`0x12`).
pub const OP_AICH_REQUEST: u8 = 0x12;

/// AICH root hash response (`0x13`).
pub const OP_AICH_ANSWER: u8 = 0x13;

/// Request AICH recovery hashset / proof tree (`0x14`).
pub const OP_AICHFILEHASHREQ: u8 = 0x14;

/// Request AICH recovery hashset / proof tree response (`0x15`).
pub const OP_AICHFILEHASHRES: u8 = 0x15;

/// Request sources from peer (SX1 - legacy, `0x81`).
pub const OP_REQUESTSOURCES: u8 = 0x81;

/// Answer sources to peer (SX1 - legacy, `0x82`).
pub const OP_ANSWERSOURCES: u8 = 0x82;

/// Request sources from peer (SX2, `0x83`).
pub const OP_REQUESTSOURCES2: u8 = 0x83;

/// Answer sources to peer (SX2, `0x84`).
pub const OP_ANSWERSOURCES2: u8 = 0x84;

/// Public Key exchange for Secure User Identification (`0x85`).
pub const OP_PUBLICKEY: u8 = 0x85;

/// Signature exchange for Secure User Identification (`0x86`).
pub const OP_SIGNATURE: u8 = 0x86;

/// Secure User Identification State (`0x87`).
pub const OP_SECIDENTSTATE: u8 = 0x87;

/// Kad UDP firewall check request (`0xA7`, sent over PR_EMULE after the TCP
/// hello). Payload: `u16 intern_kad_port` + `u16 extern_kad_port` +
/// `u32 udp_verify_key` (eMule `SendFirewallCheckUDPRequest`).
pub const OP_FWCHECKUDPREQ: u8 = 0xA7;

/// Kad firewall TCP-check acknowledgement (`0xA8`, eMule `OP_KAD_FWTCPCHECK_ACK`).
pub const OP_KAD_FWTCPCHECK_ACK: u8 = 0xA8;

// ---------------------------------------------------------------------------
// Compressed Protocol Opcodes (Protocol 0xD4)
// ---------------------------------------------------------------------------

/// Compressed data block payload (`0x40`).
pub const OP_COMPRESSEDPART: u8 = 0x40;

/// Returns a human-readable symbolic name for an opcode under a given protocol.
pub fn opcode_name(protocol: u8, opcode: u8) -> &'static str {
    match protocol {
        super::protocol::PR_ED2K => match opcode {
            OP_HELLO => "OP_HELLO / OP_LOGINREQUEST",
            OP_HELLO_ANSWER => "OP_HELLO_ANSWER",
            OP_SERVERMESSAGE => "OP_SERVERMESSAGE",
            OP_IDCHANGE => "OP_IDCHANGE",
            OP_SERVERSTATUS => "OP_SERVERSTATUS",
            OP_SERVERIDENT => "OP_SERVERIDENT",
            OP_SEARCHREQUEST => "OP_SEARCHREQUEST",
            OP_SEARCH_RESULT => "OP_SEARCH_RESULT",
            OP_GETSOURCES => "OP_GETSOURCES",
            OP_GETSOURCES_OBFU => "OP_GETSOURCES_OBFU",
            OP_FOUNDSOURCES => "OP_FOUNDSOURCES",
            OP_FOUNDSOURCES_OBFU => "OP_FOUNDSOURCES_OBFU",
            OP_OFFERFILES => "OP_OFFERFILES",
            OP_CALLBACKREQUEST => "OP_CALLBACKREQUEST",
            OP_CALLBACKREQUESTED => "OP_CALLBACKREQUESTED",
            OP_CALLBACK_FAIL => "OP_CALLBACK_FAIL",
            OP_SENDINGPART => "OP_SENDINGPART",
            OP_SETREQFILEID => "OP_SETREQFILEID",
            OP_FILEREQANSNOFILE => "OP_FILEREQANSNOFILE",
            OP_FILESTATUSREQ => "OP_FILESTATUSREQ",
            OP_REQUESTPARTS => "OP_REQUESTPARTS",
            OP_END_OF_DOWNLOAD => "OP_END_OF_DOWNLOAD",
            OP_FILESTATUS => "OP_FILESTATUS",
            OP_HASHSETREQUEST => "OP_HASHSETREQUEST",
            OP_HASHSETANSWER => "OP_HASHSETANSWER",
            OP_STARTUPLOADREQ => "OP_STARTUPLOADREQ",
            OP_ACCEPTUPLOADREQ => "OP_ACCEPTUPLOADREQ",
            OP_CANCELTRANSFER => "OP_CANCELTRANSFER",
            OP_OUTOFPARTS => "OP_OUTOFPARTS",
            OP_REQFILENAME => "OP_REQFILENAME",
            OP_REQFILENAME_ANSWER => "OP_REQFILENAME_ANSWER",
            _ => "OP_UNKNOWN_ED2K",
        },
        super::protocol::PR_EMULE => match opcode {
            OP_EMULE_INFO => "OP_EMULE_INFO",
            OP_EMULE_INFO_ANSWER => "OP_EMULE_INFO_ANSWER",
            OP_AICH_REQUEST => "OP_AICH_REQUEST",
            OP_AICH_ANSWER => "OP_AICH_ANSWER",
            OP_AICHFILEHASHREQ => "OP_AICHFILEHASHREQ",
            OP_AICHFILEHASHRES => "OP_AICHFILEHASHRES",
            OP_REQUESTSOURCES => "OP_REQUESTSOURCES",
            OP_ANSWERSOURCES => "OP_ANSWERSOURCES",
            OP_REQUESTSOURCES2 => "OP_REQUESTSOURCES2",
            OP_ANSWERSOURCES2 => "OP_ANSWERSOURCES2",
            OP_FWCHECKUDPREQ => "OP_FWCHECKUDPREQ",
            OP_KAD_FWTCPCHECK_ACK => "OP_KAD_FWTCPCHECK_ACK",
            _ => "OP_UNKNOWN_EMULE",
        },
        super::protocol::PR_EMULE_COMPRESSED => match opcode {
            OP_COMPRESSEDPART => "OP_COMPRESSEDPART",
            OP_ANSWERSOURCES => "OP_ANSWERSOURCES",
            OP_ANSWERSOURCES2 => "OP_ANSWERSOURCES2",
            _ => "OP_UNKNOWN_COMPRESSED",
        },
        _ => "OP_UNKNOWN_PROTOCOL",
    }
}
