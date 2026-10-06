//! Encumbrance signals, built from what a backend read.
//!
//! Backends do the reading; these functions only turn the result into an
//! [`EncumbranceSignal`]. They are shared and platform-independent so the
//! wording and the `present` rule are tested on every OS. A read that failed
//! must not reach here: the backend records a [`ProbeNote`] and emits no
//! signal, because an unread check is not a clean one.
//!
//! [`ProbeNote`]: crate::model::ProbeNote

use crate::model::{EncumbranceSignal, Identity, Provenance};
use crate::parse::acpi::Wpbt;

pub const ASSET_TAG: &str = "smbios_asset_tag";
pub const WPBT: &str = "wpbt";
pub const MDM_ENROLMENT: &str = "mdm_enrolment";
pub const ENTRA_JOIN: &str = "entra_join";
pub const DOMAIN_JOIN: &str = "domain_join";
pub const AUTOPILOT_PROFILE: &str = "autopilot_profile";
pub const ABSOLUTE_AGENT: &str = "absolute_agent";
pub const ACTIVATION_LOCK: &str = "activation_lock";
pub const AUTOMATED_ENROLMENT: &str = "automated_device_enrolment";
pub const FIRMWARE_PASSWORD: &str = "firmware_password";

fn signal(
    id: &str,
    present: bool,
    provenance: Provenance,
    source: impl Into<String>,
    detail: String,
) -> EncumbranceSignal {
    EncumbranceSignal {
        id: id.into(),
        present,
        provenance,
        source: source.into(),
        detail,
    }
}

/// The part of an identity after `@`, so a report names the organisation
/// without naming the previous user.
fn domain_of(upn: &str) -> Option<&str> {
    upn.rsplit_once('@')
        .map(|(_, d)| d.trim())
        .filter(|d| !d.is_empty())
}

pub fn asset_tag(identity: &Identity) -> EncumbranceSignal {
    let tag = identity.asset_tag.get();
    signal(
        ASSET_TAG,
        tag.is_some(),
        Provenance::Claimed,
        identity.asset_tag.source.clone(),
        match tag {
            Some(t) => format!("SMBIOS asset tag is set ({t:?}): ex-corporate marker"),
            None => "no SMBIOS asset tag".into(),
        },
    )
}

/// `table` is `None` when the firmware publishes no WPBT.
pub fn wpbt(table: Option<&Wpbt>, source: &str) -> EncumbranceSignal {
    let detail = match table {
        None => "firmware publishes no platform binary (WPBT)".into(),
        Some(w) => {
            let cmd = w
                .command_line
                .as_deref()
                .map(|c| format!(", command line {c:?}"))
                .unwrap_or_default();
            format!(
                "firmware hands the OS a binary to run at every boot (WPBT from {:?}{cmd}). \
                 Absolute persistence uses this to survive a disk wipe; some OEM utilities do too",
                w.oem_id
            )
        }
    };
    signal(WPBT, table.is_some(), Provenance::Measured, source, detail)
}

/// One MDM enrolment from `HKLM\SOFTWARE\Microsoft\Enrollments`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Enrolment {
    pub provider: String,
    pub upn: Option<String>,
}

pub fn mdm_enrolment(enrolments: &[Enrolment], source: &str) -> EncumbranceSignal {
    let detail = if enrolments.is_empty() {
        "no MDM enrolment recorded".into()
    } else {
        let list = enrolments
            .iter()
            .map(|e| match e.upn.as_deref().and_then(domain_of) {
                Some(d) => format!("{} for {d}", e.provider),
                None => e.provider.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!("enrolled in device management ({list}): the organisation can lock or wipe it")
    };
    signal(
        MDM_ENROLMENT,
        !enrolments.is_empty(),
        Provenance::Claimed,
        source,
        detail,
    )
}

/// One Microsoft Entra ID (Azure AD) join from `CloudDomainJoin\JoinInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntraJoin {
    pub tenant_id: String,
    pub tenant_name: Option<String>,
    pub user_email: Option<String>,
}

pub fn entra_join(joins: &[EntraJoin], source: &str) -> EncumbranceSignal {
    let detail = match joins.first() {
        None => "not joined to a Microsoft Entra ID tenant".into(),
        Some(j) => {
            let who = j
                .tenant_name
                .clone()
                .or_else(|| {
                    j.user_email
                        .as_deref()
                        .and_then(domain_of)
                        .map(String::from)
                })
                .unwrap_or_else(|| format!("tenant {}", j.tenant_id));
            format!("joined to the Microsoft Entra ID tenant of {who}: sign-in is controlled by that organisation")
        }
    };
    signal(
        ENTRA_JOIN,
        !joins.is_empty(),
        Provenance::Claimed,
        source,
        detail,
    )
}

/// `domain` is the Active Directory domain, `None` for a workgroup machine.
pub fn domain_join(domain: Option<&str>, source: &str) -> EncumbranceSignal {
    signal(
        DOMAIN_JOIN,
        domain.is_some(),
        Provenance::Claimed,
        source,
        match domain {
            Some(d) => format!("joined to Active Directory domain {d:?}: ex-corporate marker; confirm it was decommissioned"),
            None => "not joined to an Active Directory domain".into(),
        },
    )
}

/// The Autopilot profile Windows cached at setup. Absence here does not mean
/// the hardware is unregistered: registration lives in Microsoft's cloud.
pub fn autopilot_profile(tenant_domain: Option<&str>, source: &str) -> EncumbranceSignal {
    signal(
        AUTOPILOT_PROFILE,
        tenant_domain.is_some(),
        Provenance::Claimed,
        source,
        match tenant_domain {
            Some(d) => format!("Windows Autopilot profile for {d}: the machine re-enrols into that organisation after a reset"),
            None => "no Autopilot profile cached by this Windows installation".into(),
        },
    )
}

/// `found` lists the Absolute (Computrace) components present on disk or as services.
pub fn absolute_agent(found: &[String], source: &str) -> EncumbranceSignal {
    signal(
        ABSOLUTE_AGENT,
        !found.is_empty(),
        Provenance::Measured,
        source,
        if found.is_empty() {
            "no Absolute (Computrace) agent components found".into()
        } else {
            format!(
                "Absolute (Computrace) agent present ({}): the owner can track and lock the machine",
                found.join(", ")
            )
        },
    )
}

/// How Activation Lock stands on a Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationLock {
    Enabled,
    Disabled,
    /// The Mac has neither a T2 chip nor Apple silicon, so it cannot be locked.
    Unsupported,
}

pub fn activation_lock(state: ActivationLock, source: &str) -> EncumbranceSignal {
    signal(
        ACTIVATION_LOCK,
        state == ActivationLock::Enabled,
        Provenance::Measured,
        source,
        match state {
            ActivationLock::Enabled => "Activation Lock is on: the Mac is tied to the seller's Apple Account and cannot be set up after an erase without it".into(),
            ActivationLock::Disabled => "Activation Lock is off".into(),
            ActivationLock::Unsupported => "this Mac predates Activation Lock (no T2 chip or Apple silicon)".into(),
        },
    )
}

/// `enrolled` is whether macOS reports it was enrolled through Automated
/// Device Enrollment (Apple Business or School Manager, formerly DEP).
pub fn automated_enrolment(enrolled: bool, source: &str) -> EncumbranceSignal {
    signal(
        AUTOMATED_ENROLMENT,
        enrolled,
        Provenance::Claimed,
        source,
        if enrolled {
            "enrolled through Automated Device Enrollment: the Mac re-enrols into its organisation after an erase".into()
        } else {
            "not enrolled through Automated Device Enrollment".into()
        },
    )
}

/// The MDM state as macOS reports it, e.g. "Yes (User Approved)".
pub fn mdm_status(enrolled: bool, status: &str, source: &str) -> EncumbranceSignal {
    signal(
        MDM_ENROLMENT,
        enrolled,
        Provenance::Claimed,
        source,
        if enrolled {
            format!(
                "enrolled in device management ({status}): the organisation can lock or wipe it"
            )
        } else {
            "no MDM enrolment recorded".into()
        },
    )
}

pub fn firmware_password(set: bool, source: &str) -> EncumbranceSignal {
    signal(
        FIRMWARE_PASSWORD,
        set,
        Provenance::Measured,
        source,
        if set {
            "a firmware password is set: the Mac cannot boot other media or be reinstalled without it".into()
        } else {
            "no firmware password".into()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_organisation_not_the_user() {
        let s = mdm_enrolment(
            &[Enrolment {
                provider: "MS DM Server".into(),
                upn: Some("jane.doe@contoso.com".into()),
            }],
            "test",
        );
        assert!(s.present);
        assert!(s.detail.contains("contoso.com"));
        assert!(!s.detail.contains("jane"));

        let j = entra_join(
            &[EntraJoin {
                tenant_id: "72f988bf".into(),
                user_email: Some("jane.doe@contoso.com".into()),
                ..EntraJoin::default()
            }],
            "test",
        );
        assert!(j.detail.contains("contoso.com") && !j.detail.contains("jane"));
    }

    #[test]
    fn absent_signals_are_not_present() {
        assert!(!mdm_enrolment(&[], "t").present);
        assert!(!entra_join(&[], "t").present);
        assert!(!domain_join(None, "t").present);
        assert!(!autopilot_profile(None, "t").present);
        assert!(!absolute_agent(&[], "t").present);
        assert!(!wpbt(None, "t").present);
        assert!(!asset_tag(&Identity::default()).present);
        assert!(!activation_lock(ActivationLock::Disabled, "t").present);
        assert!(!activation_lock(ActivationLock::Unsupported, "t").present);
        assert!(activation_lock(ActivationLock::Enabled, "t").present);
        assert!(!automated_enrolment(false, "t").present);
        assert!(!mdm_status(false, "No", "t").present);
        assert!(!firmware_password(false, "t").present);
    }

    #[test]
    fn wpbt_names_the_publisher() {
        let w = crate::parse::acpi::parse_wpbt(&crate::parse::acpi::tests::sample()).unwrap();
        let s = wpbt(Some(&w), "t");
        assert!(s.present);
        assert!(s.detail.contains("LENOVO") && s.detail.contains("rpcnetp.exe"));
    }
}
