//! Historical operation codes (opcodes) for eD2K, eMule extensions, and compressed payloads.

// ---------------------------------------------------------------------------
// Standard eD2K Opcodes (Protocol 0xE3)
// ---------------------------------------------------------------------------

/// Handshake initialization packet (`0x01`).
pub const OP_HELLO: u8 = 0x01;
/// Client login request to server (`0x01`).
pub const OP_LOGINREQUEST: u8 = 0x01;

/// Server status response with user and file counts (`0x14`).
pub const OP_SERVERSTATUS: u8 = 0x14;

/// Server identification and metadata (`0x41`).
pub const OP_SERVERIDENT: u8 = 0x41;

/// Server message containing server greetings or notice text (`0x38`).
pub const OP_SERVERMESSAGE: u8 = 0x38;

/// Server assigns client ID and TCP flags (`0x40`).
pub const OP_IDCHANGE: u8 = 0x40;

/// Handshake response packet (`0x4C`).
pub const OP_HELLO_ANSWER: u8 = 0x4C;
/// Alias for OP_HELLO_ANSWER (`0x4C`).
pub const OP_HELLOANSWER: u8 = 0x4C;

/// Announce shared files to server (`0x15`).
pub const OP_OFFERFILES: u8 = 0x15;

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

/// AICH recovery hashset / proof tree response (`0x15`).
pub const OP_AICHFILEHASHRES: u8 = 0x15;

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
            OP_OFFERFILES => "OP_OFFERFILES",
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
            _ => "OP_UNKNOWN_EMULE",
        },
        super::protocol::PR_EMULE_COMPRESSED => match opcode {
            OP_COMPRESSEDPART => "OP_COMPRESSEDPART",
            _ => "OP_UNKNOWN_COMPRESSED",
        },
        _ => "OP_UNKNOWN_PROTOCOL",
    }
}
