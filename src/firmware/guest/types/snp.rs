// SPDX-License-Identifier: Apache-2.0

use crate::{certs::snp::ecdsa::Signature, firmware::host::TcbVersion, util::hexdump};

#[cfg(any(feature = "openssl", feature = "crypto_nossl"))]
use crate::certs::snp::{Certificate, Chain, Verifiable};

use std::fmt::Display;

#[cfg(any(feature = "openssl", feature = "crypto_nossl"))]
use std::{
    convert::TryFrom,
    io::{self, Error, ErrorKind},
};

use bitfield::bitfield;

#[cfg(feature = "openssl")]
use openssl::{ecdsa::EcdsaSig, sha::Sha384};

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// Structure of required data for fetching the derived key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedKey {
    /// Selects the root key to derive the key from.
    /// 0: Indicates VCEK.
    /// 1: Indicates VMRK.
    root_key_select: u32,

    /// Reserved, must be zero
    _reserved_0: u32,

    /// What data will be mixed into the derived key.
    pub guest_field_select: GuestFieldSelect,

    /// The VMPL to mix into the derived key. Must be greater than or equal
    /// to the current VMPL.
    pub vmpl: u32,

    /// The guest SVN to mix into the key. Must not exceed the guest SVN
    /// provided at launch in the ID block.
    pub guest_svn: u32,

    /// The TCB version to mix into the derived key. Must not
    /// exceed CommittedTcb.
    pub tcb_version: u64,
}

impl DerivedKey {
    /// Create a new instance for requesting an DerivedKey.
    pub fn new(
        root_key_select: bool,
        guest_field_select: GuestFieldSelect,
        vmpl: u32,
        guest_svn: u32,
        tcb_version: u64,
    ) -> Self {
        Self {
            root_key_select: u32::from(root_key_select),
            _reserved_0: Default::default(),
            guest_field_select,
            vmpl,
            guest_svn,
            tcb_version,
        }
    }

    /// Obtain a copy of the root key select value (Private Field)
    pub fn get_root_key_select(&self) -> u32 {
        self.root_key_select
    }
}

bitfield! {
    /// Data which will be mixed into the derived key.
    ///
    /// | Bit(s) | Name | Description |
    /// |--------|------|-------------|
    /// |0|GUEST_POLICY|Indicates that the guest policy will be mixed into the key.|
    /// |1|IMAGE_ID|Indicates that the image ID of the guest will be mixed into the key.|
    /// |2|FAMILY_ID|Indicates the family ID of the guest will be mixed into the key.|
    /// |3|MEASUREMENT|Indicates the measurement of the guest during launch will be mixed into the key.|
    /// |4|GUEST_SVN|Indicates that the guest-provided SVN will be mixed into the key.|
    /// |5|TCB_VERSION|Indicates that the guest-provided TCB_VERSION will be mixed into the key.|
    /// |63:6|\-|Reserved. Must be zero.|
    #[repr(C)]
    #[derive(Default, Copy, Clone,PartialEq, Eq, PartialOrd, Ord)]
    pub struct GuestFieldSelect(u64);
    impl Debug;
    /// Check/Set guest policy inclusion in derived key.
    pub get_guest_policy, set_guest_policy: 0, 0;
    /// Check/Set image id inclusion in derived key.
    pub get_image_id, set_image_id: 1, 1;
    /// Check/Set family id inclusion in derived key.
    pub get_family_id, set_family_id: 2, 2;
    /// Check/Set measurement inclusion in derived key.
    pub get_measurement, set_measurement: 3, 3;
    /// Check/Set svn inclusion in derived key.
    pub get_svn, set_svn: 4, 4;
    /// Check/Set tcb version inclusion in derived key.
    pub get_tcb_version, set_tcb_version: 5, 5;
}

/// The guest can request that the firmware construct an attestation report. External entities can use an
/// attestation report to assure the identity and security configuration of the guest.
///
/// A guest requests an attestation report by constructing an MSGReportReq
/// The message contains data provided by the guest in REPORT_DATA to be included
/// into the report; the firmware does not interpret this data.
///
/// Upon receiving a request for an attestation report, the PSP creates one.
///
/// The firmware generates a report ID for each guest that persists with the guest instance throughout
/// its lifetime. In each attestation report, the report ID is placed in REPORT_ID. If the guest has a
/// migration agent associated with it, the REPORT_ID_MA is filled in with the report ID of the
/// migration agent.
///
/// The firmware signs the attestation report with its VCEK. The firmware uses the system wide
/// ReportedTcb value as the TCB version to derive the VCEK. This value is set by the hypervisor.
///
/// The firmware guarantees that the ReportedTcb value is never greater than the installed TCB
/// version
#[repr(C)]
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttestationReport {
    /// Version number of this attestation report. Set to 2h for this specification.
    pub version: u32,
    /// The guest SVN.
    pub guest_svn: u32,
    /// The guest policy.
    pub policy: GuestPolicy,
    /// The family ID provided at launch.
    pub family_id: [u8; 16],
    /// The image ID provided at launch.
    pub image_id: [u8; 16],
    /// The request VMPL for the attestation report.
    pub vmpl: u32,
    /// The signature algorithm used to sign this report.
    pub sig_algo: u32,
    /// Current TCB. See SNPTcbVersion
    pub current_tcb: TcbVersion,
    /// Information about the platform. See PlatformInfo
    pub plat_info: PlatformInfo,
    /// Information related to signing keys in the report. See KeyInfo
    pub key_info: KeyInfo,
    _reserved_0: u32,
    #[serde(with = "BigArray")]
    /// Guest-provided 512 Bits of Data
    pub report_data: [u8; 64],
    #[serde(with = "BigArray")]
    /// The measurement calculated at launch.
    pub measurement: [u8; 48],
    /// Data provided by the hypervisor at launch.
    pub host_data: [u8; 32],
    #[serde(with = "BigArray")]
    /// SHA-384 digest of the ID public key that signed the ID block provided
    /// in SNP_LANUNCH_FINISH.
    pub id_key_digest: [u8; 48],
    #[serde(with = "BigArray")]
    /// SHA-384 digest of the Author public key that certified the ID key,
    /// if provided in SNP_LAUNCH_FINSIH. Zeroes if AUTHOR_KEY_EN is 1.
    pub author_key_digest: [u8; 48],
    /// Report ID of this guest.
    pub report_id: [u8; 32],
    /// Report ID of this guest's migration agent (if applicable).
    pub report_id_ma: [u8; 32],
    /// Reported TCB version used to derive the VCEK that signed this report.
    pub reported_tcb: TcbVersion,
    _reserved_1: [u8; 24],
    #[serde(with = "BigArray")]
    /// If MaskChipId is set to 0, Identifier unique to the chip.
    /// Otherwise set to 0h.
    pub chip_id: [u8; 64],
    /// CommittedTCB
    pub committed_tcb: TcbVersion,
    /// The build number of CurrentVersion
    pub current_build: u8,
    /// The minor number of CurrentVersion
    pub current_minor: u8,
    /// The major number of CurrentVersion
    pub current_major: u8,
    _reserved_2: u8,
    /// The build number of CommittedVersion
    pub committed_build: u8,
    /// The minor number of CommittedVersion
    pub committed_minor: u8,
    /// The major number of CommittedVersion
    pub committed_major: u8,
    _reserved_3: u8,
    /// The CurrentTcb at the time the guest was launched or imported.
    pub launch_tcb: TcbVersion,
    #[serde(with = "BigArray")]
    _reserved_4: [u8; 168],
    /// Signature of bytes 0 to 0x29F inclusive of this report.
    /// The format of the signature is found within Signature.
    pub signature: Signature,
}

impl Default for AttestationReport {
    fn default() -> Self {
        Self {
            version: Default::default(),
            guest_svn: Default::default(),
            policy: Default::default(),
            family_id: Default::default(),
            image_id: Default::default(),
            vmpl: Default::default(),
            sig_algo: Default::default(),
            current_tcb: Default::default(),
            plat_info: Default::default(),
            key_info: Default::default(),
            _reserved_0: Default::default(),
            report_data: [0; 64],
            measurement: [0; 48],
            host_data: Default::default(),
            id_key_digest: [0; 48],
            author_key_digest: [0; 48],
            report_id: Default::default(),
            report_id_ma: Default::default(),
            reported_tcb: Default::default(),
            _reserved_1: Default::default(),
            chip_id: [0; 64],
            committed_tcb: Default::default(),
            current_build: Default::default(),
            current_minor: Default::default(),
            current_major: Default::default(),
            _reserved_2: Default::default(),
            committed_build: Default::default(),
            committed_minor: Default::default(),
            committed_major: Default::default(),
            _reserved_3: Default::default(),
            launch_tcb: Default::default(),
            _reserved_4: [0; 168],
            signature: Default::default(),
        }
    }
}

impl Display for AttestationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
Attestation Report ({} bytes):
Version:                      {}
Guest SVN:                    {}
{}
Family ID:                    {}
Image ID:                     {}
VMPL:                         {}
Signature Algorithm:          {}
Current TCB:
{}
{}
{}
Report Data:                  {}
Measurement:                  {}
Host Data:                    {}
ID Key Digest:                {}
Author Key Digest:            {}
Report ID:                    {}
Report ID Migration Agent:    {}
Reported TCB:                 {}
Chip ID:                      {}
Committed TCB:
{}
Current Build:                {}
Current Minor:                {}
Current Major:                {}
Committed Build:              {}
Committed Minor:              {}
Committed Major:              {}
Launch TCB:
{}
{}
"#,
            std::mem::size_of_val(self),
            self.version,
            self.guest_svn,
            self.policy,
            hexdump(&self.family_id),
            hexdump(&self.image_id),
            self.vmpl,
            self.sig_algo,
            self.current_tcb,
            self.plat_info,
            self.key_info,
            hexdump(&self.report_data),
            hexdump(&self.measurement),
            hexdump(&self.host_data),
            hexdump(&self.id_key_digest),
            hexdump(&self.author_key_digest),
            hexdump(&self.report_id),
            hexdump(&self.report_id_ma),
            self.reported_tcb,
            hexdump(&self.chip_id),
            self.committed_tcb,
            self.current_build,
            self.current_minor,
            self.current_major,
            self.committed_build,
            self.committed_minor,
            self.committed_major,
            self.launch_tcb,
            self.signature
        )
    }
}

#[cfg(feature = "openssl")]
impl Verifiable for (&Chain, &AttestationReport) {
    type Output = ();

    fn verify(self) -> io::Result<Self::Output> {
        let vek = self.0.verify()?;

        let sig = EcdsaSig::try_from(&self.1.signature)?;
        let measurable_bytes: &[u8] = &bincode::serialize(self.1).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("Unable to serialize bytes: {}", e),
            )
        })?[..0x2a0];

        let mut hasher = Sha384::new();
        hasher.update(measurable_bytes);
        let base_digest = hasher.finish();

        let ec = vek.public_key()?.ec_key()?;
        let signed = sig.verify(&base_digest, &ec)?;

        match signed {
            true => Ok(()),
            false => Err(Error::new(
                ErrorKind::Other,
                "VEK does not sign the attestation report",
            )),
        }
    }
}

#[cfg(feature = "openssl")]
impl Verifiable for (&Certificate, &AttestationReport) {
    type Output = ();

    fn verify(self) -> io::Result<Self::Output> {
        let vek = self.0;

        let sig = EcdsaSig::try_from(&self.1.signature)?;
        let measurable_bytes: &[u8] = &bincode::serialize(self.1).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("Unable to serialize bytes: {}", e),
            )
        })?[..0x2a0];

        let mut hasher = Sha384::new();
        hasher.update(measurable_bytes);
        let base_digest = hasher.finish();

        let ec = vek.public_key()?.ec_key()?;
        let signed = sig.verify(&base_digest, &ec)?;

        match signed {
            true => Ok(()),
            false => Err(Error::new(
                ErrorKind::Other,
                "VEK does not sign the attestation report",
            )),
        }
    }
}

#[cfg(feature = "crypto_nossl")]
impl Verifiable for (&Chain, &AttestationReport) {
    type Output = ();

    fn verify(self) -> io::Result<Self::Output> {
        // According to Chapter 3 of the Versioned Chip Endorsement Key (VCEK) Certificate and the Versioned Loaded Endorsement Key (VLEK)
        // Certificate specifications, both Versioned Endorsement Key certificates certify an ECDSA public key on curve P-384,
        // with the signature hash algorithm being SHA-384.
        let vek = self.0.verify()?;

        let sig = p384::ecdsa::Signature::try_from(&self.1.signature)?;

        let measurable_bytes: &[u8] = &bincode::serialize(self.1).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("Unable to serialize bytes: {}", e),
            )
        })?[..0x2a0];

        use sha2::Digest;
        let base_digest = sha2::Sha384::new_with_prefix(measurable_bytes);

        let verifying_key = p384::ecdsa::VerifyingKey::from_sec1_bytes(vek.public_key_sec1())
            .map_err(|e| {
                io::Error::new(
                    ErrorKind::Other,
                    format!("failed to deserialize public key from sec1 bytes: {e:?}"),
                )
            })?;

        use p384::ecdsa::signature::DigestVerifier;
        verifying_key.verify_digest(base_digest, &sig).map_err(|e| {
            io::Error::new(
                ErrorKind::Other,
                format!("VCEK does not sign the attestation report: {e:?}"),
            )
        })
    }
}

#[cfg(feature = "crypto_nossl")]
impl Verifiable for (&Certificate, &AttestationReport) {
    type Output = ();

    fn verify(self) -> io::Result<Self::Output> {
        // According to Chapter 3 of the [Versioned Chip Endorsement Key (VCEK) Certificate and
        // KDS Interface Specification][spec], the VCEK certificate certifies an ECDSA public key on curve P-384,
        // and the signature hash algorithm is sha384.
        // [spec]: https://www.amd.com/content/dam/amd/en/documents/epyc-technical-docs/specifications/57230.pdf

        let vek = self.0;

        let sig = p384::ecdsa::Signature::try_from(&self.1.signature)?;

        let measurable_bytes: &[u8] = &bincode::serialize(self.1).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("Unable to serialize bytes: {}", e),
            )
        })?[..0x2a0];

        use sha2::Digest;
        let base_digest = sha2::Sha384::new_with_prefix(measurable_bytes);

        let verifying_key = p384::ecdsa::VerifyingKey::from_sec1_bytes(vek.public_key_sec1())
            .map_err(|e| {
                io::Error::new(
                    ErrorKind::Other,
                    format!("failed to deserialize public key from sec1 bytes: {e:?}"),
                )
            })?;

        use p384::ecdsa::signature::DigestVerifier;
        verifying_key.verify_digest(base_digest, &sig).map_err(|e| {
            io::Error::new(
                ErrorKind::Other,
                format!("VCEK does not sign the attestation report: {e:?}"),
            )
        })
    }
}

bitfield! {
    /// The firmware associates each guest with a guest policy that the guest owner provides. The
    /// firmware restricts what actions the hypervisor can take on this guest according to the guest policy.
    /// The policy also indicates the minimum firmware version to for the guest.
    ///
    /// The guest owner provides the guest policy to the firmware during launch. The firmware then binds
    /// the policy to the guest. The policy cannot be changed throughout the lifetime of the guest. The
    /// policy is also migrated with the guest and enforced by the destination platform firmware.
    ///
    /// | Bit(s) | Name              | Description                                                                                                        >
    /// |--------|-------------------|-------------------------------------------------------------------------------------------------------------------->
    /// | 7:0    | ABI_MINOR         | The minimum ABI minor version required for this guest to run.                                                      >
    /// | 15:8   | ABI_MAJOR         | The minimum ABI major version required for this guest to run.                                                      >
    /// | 16     | SMT               | 0: Host SMT usage is disallowed.<br>1: Host SMT usage is allowed.                                                  >
    /// | 17     | -                 | Reserved. Must be one.                                                                                             >
    /// | 18     | MIGRATE_MA        | 0: Association with a migration agent is disallowed.<br>1: Association with a migration agent is allowed           >
    /// | 19     | DEBUG             | 0: Debugging is disallowed.<br>1: Debugging is allowed.                                                            >
    /// | 20     | SINGLE_SOCKET     | 0: Guest can be activated on multiple sockets.<br>1: Guest can only be activated on one socket.                    >
    /// | 21     | CXL_ALLOW         | 0: CXL cannot be populated with devices or memory.<br>1: CXL can be populated with devices or memory.              >
    /// | 22     | MEM_AES_256_XTS   | 0: Allow either AES 128 XEX or AES 256 XTS for memory encryption.<br>1: Require AES 256 XTS for memory encryption. >
    /// | 23     | RAPL_DIS          | 0: Allow Running Average Power Limit (RAPL).<br>1: RAPL must be disabled.                                          >
    /// | 24     | CIPHERTEXT_HIDING | 0: Ciphertext hiding may be enabled or disabled.<br>1: Ciphertext hiding must be enabled.                          >
    /// | 63:25  | -                 | Reserved. MBZ.                                                                                                     >
    ///
    #[repr(C)]
    #[derive(Default, Deserialize, Clone, Copy, Eq, PartialEq, Serialize, PartialOrd, Ord)]
    pub struct GuestPolicy(u64);
    impl Debug;
    /// ABI_MINOR field: Indicates the minor API version.
    pub abi_minor, set_abi_minor: 7, 0;
    /// ABI_MAJOR field: Indicates the minor API version.
    pub abi_major, set_abi_major: 15, 8;
    /// SMT_ALLOWED field: Indicates the if SMT should be permitted.
    pub smt_allowed, set_smt_allowed: 16, 16;
    /// MIGRATE_MA_ALLOWED field: Indicates the if migration is permitted with
    /// the migration agent.
    pub migrate_ma_allowed, set_migrate_ma_allowed: 18, 18;
    /// DEBUG_ALLOWED field: Indicates the if debugging should is permitted.
    pub debug_allowed, set_debug_allowed: 19, 19;
    /// SINGLE_SOCKET_REQUIRED field: Indicates the if a single socket is required.
    pub single_socket_required, set_single_socket_required: 20, 20;
    /// CXL_ALLOW field: (1) can populate CXL devices/memory, (0) cannot populate CXL devices/memory
    pub cxl_allowed, set_cxl_allowed: 21, 21;
    /// MEM_AES_256_XTS field: (1) require AES 256 XTS encryption, (0) allows either AES 128 XEX or AES 256 XTS encryption
    pub mem_aes_256_xts, set_mem_aes_256_xts: 22, 22;
    /// RAPL_DIS field: (1) RAPL must be disabled, (0) allow RAPL
    pub rapl_dis, set_rapl_dis: 23, 23;
    /// CIPHERTEXT_HIDING field: (1) ciphertext hiding must be enabled, (0) ciphertext hiding may be enabled/disabled
    pub ciphertext_hiding, set_ciphertext_hiding: 24, 24;
}

impl Display for GuestPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
    Guest Policy (0x{:x}):
    ABI Major:     {}
    ABI Minor:     {}
    SMT Allowed:   {}
    Migrate MA:    {}
    Debug Allowed: {}
    Single Socket: {}"#,
            self.0,
            self.abi_major(),
            self.abi_minor(),
            self.smt_allowed(),
            self.migrate_ma_allowed(),
            self.debug_allowed(),
            self.single_socket_required()
        )
    }
}

impl From<GuestPolicy> for u64 {
    fn from(value: GuestPolicy) -> Self {
        // Bit 17 of the guest policy is reserved and must always be set to 1.
        let reserved: u64 = 1 << 17;

        value.0 | reserved
    }
}

bitfield! {
    /// A structure with a bit-field unsigned 64 bit integer:
    /// Bit 0 representing the status of SMT enablement.
    /// Bit 1 representing the status of TSME enablement.
    /// Bit 2 indicates if ECC memory is used.
    /// Bit 3 indicates if RAPL is disabled.
    /// Bit 4 indicates if ciphertext hiding is enabled
    /// Bits 5-63 are reserved.
    #[repr(C)]
    #[derive(Default, Deserialize, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct PlatformInfo(u64);
    impl Debug;
    /// Returns the bit state of SMT
    pub smt_enabled, _: 0, 0;
    /// Returns the bit state of TSME.
    pub tsme_enabled, _: 1, 1;
    /// Indicates that the platform is currently using ECC memory
    pub ecc_enabled, _: 2, 2;
    /// Indicates that the RAPL feature is disabled
    pub rapl_disabled, _: 3, 3;
    /// Indicates that ciphertext hiding is enabled
    pub ciphertext_hiding_enabled, _: 4, 4;
    /// reserved
    reserved, _: 63, 5;
}

impl Display for PlatformInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
Platform Info ({}):
  SMT Enabled:               {}
  TSME Enabled:              {}
  ECC Enabled:               {}
  RAPL Disabled:             {}
  Ciphertext Hiding Enabled: {}
"#,
            self.0,
            self.smt_enabled(),
            self.tsme_enabled(),
            self.ecc_enabled(),
            self.rapl_disabled(),
            self.ciphertext_hiding_enabled(),
        )
    }
}

bitfield! {
    /// When an attestation report is requested, the user can request to have the report to not be signed, or sign with different keys. The user may also
    /// pass in the author key when launching the guest. This field provides that information and will be present in the attestation report.
    ///
    /// | Bit(s) | Name              | Description                                                                                                        >
    /// |--------|-------------------|-------------------------------------------------------------------------------------------------------------------->
    /// | 0      | AUTHOR_KEY_EN     | Indicates that the digest of the author key is present in AUTHOR_KEY_DIGEST. Set to the value of GCTX.AuthorKeyEn. >
    /// | 1      | MASK_CHIP_KEY     | The value of MaskChipKey.                                                                                          >
    /// | 4:2    | SIGNING_KEY       | Encodes the key used to sign this report.                                                                          >
    /// | 5:31   | -                 | Reserved. Must be zero.                                                                                            >
    #[repr(C)]
    #[derive(Default, Deserialize, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Serialize)]
    pub struct KeyInfo(u32);
    impl Debug;
    /// AUTHOR_KEY_EN field: Indicates that the digest of the author key is present in AUTHOR_KEY_DIGEST
    pub author_key_en, _: 0;
    /// MASK_CHIP_KEY field: The value of MaskChipKey
    /// (0) Firmware signs the attestation report with either the VCEK OR VLEK.
    /// (1) The firmware writes 0s into the SIGNATURE field instead of signing the report.
    pub mask_chip_key, _: 1,1;
    /// SIGNING_KEY field: Encodes the key used to sign this report.
    /// (0) VCEK
    /// (1) VLEK
    /// (2-6) RESERVED
    /// (7) NONE
    pub signing_key, _: 4,2;
    /// reserved
    reserved, _: 31, 5;
}

impl Display for KeyInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let signing_key = match self.signing_key() {
            0 => "vcek",
            1 => "vlek",
            7 => "none",
            _ => "unkown",
        };

        write!(
            f,
            r#"
Key Information:
    author key enabled: {}
    mask chip key:      {}
    signing key:        {}
"#,
            self.author_key_en(),
            self.mask_chip_key(),
            signing_key
        )
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_derive_key_new() {
        let expected: DerivedKey = DerivedKey {
            root_key_select: 0,
            _reserved_0: 0,
            guest_field_select: GuestFieldSelect(0),
            vmpl: 0,
            guest_svn: 0,
            tcb_version: 0,
        };

        let guest_field: GuestFieldSelect = GuestFieldSelect(0);

        let actual: DerivedKey = DerivedKey::new(false, guest_field, 0, 0, 0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_derive_key_get_root_key_select() {
        let dk_struct: DerivedKey = DerivedKey {
            root_key_select: 0,
            _reserved_0: 0,
            guest_field_select: GuestFieldSelect(0),
            vmpl: 0,
            guest_svn: 0,
            tcb_version: 0,
        };

        let expected: u32 = 0;
        let actual: u32 = dk_struct.get_root_key_select();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_guest_field_select_all_on() {
        let actual: GuestFieldSelect = GuestFieldSelect(0b111111);

        assert_eq!(actual.get_guest_policy(), 1);
        assert_eq!(actual.get_image_id(), 1);
        assert_eq!(actual.get_family_id(), 1);
        assert_eq!(actual.get_measurement(), 1);
        assert_eq!(actual.get_svn(), 1);
        assert_eq!(actual.get_tcb_version(), 1);
    }

    #[test]
    fn test_guest_field_select_all_off() {
        let actual: GuestFieldSelect = GuestFieldSelect(0);

        assert_eq!(actual.get_guest_policy(), 0);
        assert_eq!(actual.get_image_id(), 0);
        assert_eq!(actual.get_family_id(), 0);
        assert_eq!(actual.get_measurement(), 0);
        assert_eq!(actual.get_svn(), 0);
        assert_eq!(actual.get_tcb_version(), 0);
    }

    #[test]
    fn test_attestation_report() {
        let expected: AttestationReport = AttestationReport {
            version: 0,
            guest_svn: 0,
            policy: GuestPolicy(0),
            family_id: [0; 16],
            image_id: [0; 16],
            vmpl: 0,
            sig_algo: 0,
            current_tcb: TcbVersion::default(),
            plat_info: PlatformInfo::default(),
            key_info: KeyInfo::default(),
            _reserved_0: 0,
            report_data: [0; 64],
            measurement: [0; 48],
            host_data: [0; 32],
            id_key_digest: [0; 48],
            author_key_digest: [0; 48],
            report_id: [0; 32],
            report_id_ma: [0; 32],
            reported_tcb: TcbVersion::default(),
            _reserved_1: [0; 24],
            chip_id: [0; 64],
            committed_tcb: TcbVersion::default(),
            current_build: 0,
            current_minor: 0,
            current_major: 0,
            _reserved_2: 0,
            committed_build: 0,
            committed_minor: 0,
            committed_major: 0,
            _reserved_3: 0,
            launch_tcb: TcbVersion::default(),
            _reserved_4: [0; 168],
            signature: Signature::default(),
        };

        assert_eq!(AttestationReport::default(), expected);
    }

    #[test]
    fn test_attestation_report_default() {
        let expected: AttestationReport = AttestationReport {
            version: Default::default(),
            guest_svn: Default::default(),
            policy: GuestPolicy::default(),
            family_id: Default::default(),
            image_id: Default::default(),
            vmpl: Default::default(),
            sig_algo: Default::default(),
            current_tcb: TcbVersion::default(),
            plat_info: PlatformInfo::default(),
            key_info: KeyInfo::default(),
            _reserved_0: Default::default(),
            report_data: [0; 64],
            measurement: [0; 48],
            host_data: Default::default(),
            id_key_digest: [0; 48],
            author_key_digest: [0; 48],
            report_id: Default::default(),
            report_id_ma: Default::default(),
            reported_tcb: TcbVersion::default(),
            _reserved_1: Default::default(),
            chip_id: [0; 64],
            committed_tcb: TcbVersion::default(),
            current_build: Default::default(),
            current_minor: Default::default(),
            current_major: Default::default(),
            _reserved_2: Default::default(),
            committed_build: Default::default(),
            committed_minor: Default::default(),
            committed_major: Default::default(),
            _reserved_3: Default::default(),
            launch_tcb: TcbVersion::default(),
            _reserved_4: [0; 168],
            signature: Signature::default(),
        };

        assert_eq!(AttestationReport::default(), expected);
    }

    #[test]
    fn test_attestation_report_fmt() {
        let expected: &str = r#"
Attestation Report (1184 bytes):
Version:                      0
Guest SVN:                    0

    Guest Policy (0x0):
    ABI Major:     0
    ABI Minor:     0
    SMT Allowed:   0
    Migrate MA:    0
    Debug Allowed: 0
    Single Socket: 0
Family ID:                    
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Image ID:                     
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

VMPL:                         0
Signature Algorithm:          0
Current TCB:

TCB Version:
  Microcode:   0
  SNP:         0
  TEE:         0
  Boot Loader: 0
  

Platform Info (0):
  SMT Enabled:               0
  TSME Enabled:              0
  ECC Enabled:               0
  RAPL Disabled:             0
  Ciphertext Hiding Enabled: 0


Key Information:
    author key enabled: false
    mask chip key:      0
    signing key:        vcek

Report Data:                  
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Measurement:                  
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Host Data:                    
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

ID Key Digest:                
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Author Key Digest:            
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Report ID:                    
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Report ID Migration Agent:    
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Reported TCB:                 
TCB Version:
  Microcode:   0
  SNP:         0
  TEE:         0
  Boot Loader: 0
  
Chip ID:                      
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 

Committed TCB:

TCB Version:
  Microcode:   0
  SNP:         0
  TEE:         0
  Boot Loader: 0
  
Current Build:                0
Current Minor:                0
Current Major:                0
Committed Build:              0
Committed Minor:              0
Committed Major:              0
Launch TCB:

TCB Version:
  Microcode:   0
  SNP:         0
  TEE:         0
  Boot Loader: 0
  

Signature:
  R: 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 

  S: 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 

            
"#;

        assert_eq!(expected, AttestationReport::default().to_string())
    }

    #[test]
    fn test_attestation_report_clone() {
        let expected: AttestationReport = AttestationReport::default();

        let copy: AttestationReport = expected;

        assert_eq!(expected, copy);
    }

    #[test]
    fn test_attestation_report_copy() {
        let expected: AttestationReport = AttestationReport::default();

        let copy: AttestationReport = expected;

        assert_eq!(expected, copy);
    }

    #[test]
    fn test_guest_policy_zeroed() {
        let gp: GuestPolicy = GuestPolicy(0);

        assert_eq!(gp.abi_minor(), 0);
        assert_eq!(gp.abi_major(), 0);
        assert_eq!(gp.smt_allowed(), 0);
        assert_eq!(gp.migrate_ma_allowed(), 0);
        assert_eq!(gp.debug_allowed(), 0);
        assert_eq!(gp.single_socket_required(), 0);
        assert_eq!(gp.cxl_allowed(), 0);
        assert_eq!(gp.mem_aes_256_xts(), 0);
        assert_eq!(gp.rapl_dis(), 0);
        assert_eq!(gp.ciphertext_hiding(), 0);
    }

    #[test]
    fn test_guest_policy_max() {
        let gp: GuestPolicy = GuestPolicy(0b1111111111111111111111111);

        assert_eq!(gp.abi_minor(), 0b11111111);
        assert_eq!(gp.abi_major(), 0b11111111);
        assert_eq!(gp.smt_allowed(), 1);
        assert_eq!(gp.migrate_ma_allowed(), 1);
        assert_eq!(gp.debug_allowed(), 1);
        assert_eq!(gp.single_socket_required(), 1);
        assert_eq!(gp.cxl_allowed(), 1);
        assert_eq!(gp.mem_aes_256_xts(), 1);
        assert_eq!(gp.rapl_dis(), 1);
        assert_eq!(gp.ciphertext_hiding(), 1);
    }

    #[test]
    fn test_set_guest_policy_max() {
        let mut gp: GuestPolicy = GuestPolicy::default();

        assert_eq!(gp.abi_minor(), 0);
        gp.set_abi_minor(1);
        assert_eq!(gp.abi_minor(), 0b1);

        assert_eq!(gp.abi_major(), 0);
        gp.set_abi_major(1);
        assert_eq!(gp.abi_major(), 0b1);

        assert_eq!(gp.smt_allowed(), 0);
        gp.set_smt_allowed(1);
        assert_eq!(gp.smt_allowed(), 1);

        assert_eq!(gp.migrate_ma_allowed(), 0);
        gp.set_migrate_ma_allowed(1);
        assert_eq!(gp.migrate_ma_allowed(), 1);

        assert_eq!(gp.debug_allowed(), 0);
        gp.set_debug_allowed(1);
        assert_eq!(gp.debug_allowed(), 1);

        assert_eq!(gp.single_socket_required(), 0);
        gp.set_single_socket_required(1);
        assert_eq!(gp.single_socket_required(), 1);

        assert_eq!(gp.cxl_allowed(), 0);
        gp.set_cxl_allowed(1);
        assert_eq!(gp.cxl_allowed(), 1);

        assert_eq!(gp.mem_aes_256_xts(), 0);
        gp.set_mem_aes_256_xts(1);
        assert_eq!(gp.mem_aes_256_xts(), 1);

        assert_eq!(gp.rapl_dis(), 0);
        gp.set_rapl_dis(1);
        assert_eq!(gp.rapl_dis(), 1);

        assert_eq!(gp.ciphertext_hiding(), 0);
        gp.set_ciphertext_hiding(1);
        assert_eq!(gp.ciphertext_hiding(), 1);
    }

    #[test]
    fn test_guest_policy_from_u64() {
        let gp: GuestPolicy = GuestPolicy(5);

        // Bit 17 of the guest policy is reserved and must always be set to 1.
        let expected: u64 = (1 << 17) | 5;

        assert_eq!(u64::from(gp), expected);
    }

    #[test]
    fn test_platform_info_zeroed() {
        let expected: PlatformInfo = PlatformInfo(0);

        assert_eq!(expected.smt_enabled(), 0);
        assert_eq!(expected.tsme_enabled(), 0);
        assert_eq!(expected.ecc_enabled(), 0);
        assert_eq!(expected.rapl_disabled(), 0);
        assert_eq!(expected.ciphertext_hiding_enabled(), 0);
    }

    #[test]
    fn test_platform_info_full() {
        let expected: PlatformInfo = PlatformInfo(0b11111);

        assert_eq!(expected.smt_enabled(), 1);
        assert_eq!(expected.tsme_enabled(), 1);
        assert_eq!(expected.ecc_enabled(), 1);
        assert_eq!(expected.rapl_disabled(), 1);
        assert_eq!(expected.ciphertext_hiding_enabled(), 1);
    }

    #[test]
    fn test_platform_info_fmt() {
        let expected: &str = r#"
Platform Info (0):
  SMT Enabled:               0
  TSME Enabled:              0
  ECC Enabled:               0
  RAPL Disabled:             0
  Ciphertext Hiding Enabled: 0
"#;
        let actual: PlatformInfo = PlatformInfo(0);

        assert_eq!(expected, actual.to_string());
    }

    #[test]
    fn test_key_info_zeroed() {
        let expected: KeyInfo = KeyInfo(0);

        assert!(!expected.author_key_en());
        assert_eq!(expected.mask_chip_key(), 0);

        assert_eq!(expected.signing_key(), 0);
    }

    #[test]
    fn test_key_info_max() {
        let expected: KeyInfo = KeyInfo(0b11111);

        assert!(expected.author_key_en());
        assert_eq!(expected.mask_chip_key(), 1);
        assert_eq!(expected.signing_key(), 0b111);
    }

    #[test]
    fn test_key_info_fmt_vcek() {
        let expected: &str = r#"
Key Information:
    author key enabled: false
    mask chip key:      0
    signing key:        vcek
"#;
        let actual: KeyInfo = KeyInfo(0);

        assert_eq!(expected, actual.to_string());
    }

    #[test]
    fn test_key_info_fmt_vlek() {
        let expected: &str = r#"
Key Information:
    author key enabled: false
    mask chip key:      0
    signing key:        vlek
"#;
        let actual: KeyInfo = KeyInfo(0b100);

        assert_eq!(expected, actual.to_string());
    }

    #[test]
    fn test_key_info_fmt_none() {
        let expected: &str = r#"
Key Information:
    author key enabled: false
    mask chip key:      0
    signing key:        none
"#;
        let actual: KeyInfo = KeyInfo(0b11100);

        assert_eq!(expected, actual.to_string());
    }

    #[test]
    fn test_key_info_fmt_unknown() {
        let expected: &str = r#"
Key Information:
    author key enabled: false
    mask chip key:      0
    signing key:        unkown
"#;
        let actual: KeyInfo = KeyInfo(0b11000);

        assert_eq!(expected, actual.to_string());
    }

    #[test]
    fn test_platform_info_serialization() {
        let original = PlatformInfo(0b11111);

        // Test bincode
        let binary = bincode::serialize(&original).unwrap();
        let from_binary: PlatformInfo = bincode::deserialize(&binary).unwrap();
        assert_eq!(original, from_binary);
    }

    #[test]
    fn test_key_info_serialization() {
        let original = KeyInfo(0b11111);

        // Test bincode
        let binary = bincode::serialize(&original).unwrap();
        let from_binary: KeyInfo = bincode::deserialize(&binary).unwrap();
        assert_eq!(original, from_binary);
        assert!(from_binary.author_key_en());
        assert_eq!(from_binary.mask_chip_key(), 1);
        assert_eq!(from_binary.signing_key(), 0b111);
    }

    #[test]
    fn test_guest_policy_serialization() {
        let mut original = GuestPolicy::default();
        original.set_abi_major(2);
        original.set_abi_minor(1);
        original.set_smt_allowed(1);
        original.set_debug_allowed(1);

        // Test bincode
        let binary = bincode::serialize(&original).unwrap();
        let from_binary: GuestPolicy = bincode::deserialize(&binary).unwrap();
        assert_eq!(original, from_binary);
    }

    #[test]
    fn test_attestation_report_serialization() {
        let original: AttestationReport = AttestationReport {
            version: 2,
            guest_svn: 1,
            policy: GuestPolicy(3),
            family_id: [1; 16],
            image_id: [2; 16],
            ..Default::default()
        };

        // Test bincode
        let binary = bincode::serialize(&original).unwrap();
        let from_binary: AttestationReport = bincode::deserialize(&binary).unwrap();
        assert_eq!(original, from_binary);
    }

    #[test]
    fn test_boundary_value_serialization() {
        // Test max values
        let platform_info = PlatformInfo(u64::MAX);
        let key_info = KeyInfo(u32::MAX);
        let guest_policy = GuestPolicy(u64::MAX);

        // Verify serialization/deserialization preserves max values
        assert_eq!(
            platform_info,
            bincode::deserialize(&bincode::serialize(&platform_info).unwrap()).unwrap()
        );
        assert_eq!(
            key_info,
            bincode::deserialize(&bincode::serialize(&key_info).unwrap()).unwrap()
        );
        assert_eq!(
            guest_policy,
            bincode::deserialize(&bincode::serialize(&guest_policy).unwrap()).unwrap()
        );
    }

    #[test]
    fn test_guest_field_select_operations() {
        let mut field = GuestFieldSelect::default();

        field.set_guest_policy(1);
        assert_eq!(field.get_guest_policy(), 1);

        field.set_image_id(1);
        assert_eq!(field.get_image_id(), 1);

        field.set_family_id(1);
        assert_eq!(field.get_family_id(), 1);

        field.set_measurement(1);
        assert_eq!(field.get_measurement(), 1);
    }

    #[test]
    fn test_derived_key_fields() {
        let key = DerivedKey::new(true, GuestFieldSelect(0xFF), 2, 3, 0x1234);
        assert_eq!(key.get_root_key_select(), 1);
        assert_eq!(key.vmpl, 2);
        assert_eq!(key.guest_svn, 3);
        assert_eq!(key.tcb_version, 0x1234);
    }

    #[test]
    fn test_key_info_all_combinations() {
        let mut info = KeyInfo(0);

        // Test VCEK
        assert_eq!(info.signing_key(), 0);
        assert!(!info.author_key_en());

        // Test VLEK
        info = KeyInfo(0b100);
        assert_eq!(info.signing_key(), 1);

        // Test None
        info = KeyInfo(0b11100);
        assert_eq!(info.signing_key(), 7);
    }

    #[test]
    fn test_attestation_report_fields() {
        let report: AttestationReport = AttestationReport {
            version: 2,
            guest_svn: 1,
            vmpl: 3,
            ..Default::default()
        };
        assert_eq!(report.version, 2);
        assert_eq!(report.guest_svn, 1);
        assert_eq!(report.vmpl, 3);
        assert_eq!(report.measurement, [0; 48]);
    }

    #[test]
    fn test_platform_info_reserved() {
        let info = PlatformInfo(0xFF);
        assert_eq!(info.reserved(), 0x7);
    }

    #[test]
    fn test_guest_policy_combined_fields() {
        let mut policy = GuestPolicy::default();

        policy.set_abi_major(2);
        policy.set_abi_minor(1);
        policy.set_smt_allowed(1);
        policy.set_debug_allowed(1);

        assert_eq!(policy.abi_major(), 2);
        assert_eq!(policy.abi_minor(), 1);
        assert_eq!(policy.smt_allowed(), 1);
        assert_eq!(policy.debug_allowed(), 1);

        let policy_u64: u64 = policy.into();
        assert_eq!(policy_u64 & (1 << 17), 1 << 17); // Reserved bit 17 must be 1
    }
}
