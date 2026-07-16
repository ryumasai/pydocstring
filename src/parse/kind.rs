//! Style-independent section-name resolution (#148; the enum half of the
//! shared block dispatcher the old per-style `kind.rs` twins pointed at).
//!
//! One enum covers every section kind either dialect recognizes — the two
//! dialects never disagreed on the *kinds*, only on the alias strings that
//! reach them. Those alias tables stay separate ([`from_google_name`] /
//! [`from_numpy_name`]) because they are snapshot-pinned behavior: Google
//! grandfathers historic typo aliases (`argment`, `paramter`), NumPy accepts
//! the singular full forms (`parameter`, `argument`). Everything else —
//! role routing and the mapping to the model's
//! [`SectionKind`](crate::model::SectionKind) — is shared.
//!
//! [`from_google_name`]: SectionName::from_google_name
//! [`from_numpy_name`]: SectionName::from_numpy_name

use crate::model::FreeSectionKind;
use crate::model::SectionKind;
use crate::parse::EntryRole;

/// A recognized section name, style-independent.
///
/// `Args:` (Google) and `Parameters` + underline (NumPy) both resolve to
/// [`Parameters`](Self::Parameters); unrecognised names resolve to
/// [`Unknown`](Self::Unknown). Convert lowercased header text with the
/// per-style constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum SectionName {
    /// `Args` / `Arguments` / `Parameters` / `Params` …
    Parameters,
    /// `Keyword Args` / `Keyword Arguments` / `Keyword Parameters` …
    KeywordParameters,
    /// `Other Parameters` / `Other Args` …
    OtherParameters,
    /// `Receives` / `Receive`
    Receives,
    /// `Returns` / `Return`
    Returns,
    /// `Yields` / `Yield`
    Yields,
    /// `Raises` / `Raise`
    Raises,
    /// `Warns` / `Warn`
    Warns,
    /// `Attributes` / `Attribute`
    Attributes,
    /// `Methods` / `Method`
    Methods,
    /// `See Also`
    SeeAlso,
    /// `Notes` / `Note`
    Notes,
    /// `Examples` / `Example`
    Examples,
    /// `Todo`
    Todo,
    /// `References` / `Reference`
    References,
    /// `Warnings` / `Warning`
    Warnings,
    /// `Attention`
    Attention,
    /// `Caution`
    Caution,
    /// `Danger`
    Danger,
    /// `Error`
    Error,
    /// `Hint`
    Hint,
    /// `Important`
    Important,
    /// `Tip`
    Tip,
    /// Unrecognised section name.
    Unknown,
}

impl SectionName {
    /// Aliases both dialects accept, **lowercased**.
    #[rustfmt::skip]
    fn from_common_name(name: &str) -> Self {
        match name {
            "args" | "arg" | "arguments" => Self::Parameters,
            "params" | "param" | "parameters" => Self::Parameters,
            "keyword args" | "keyword arg" | "keyword arguments" | "keyword argument" => Self::KeywordParameters,
            "keyword params" | "keyword param" | "keyword parameters" => Self::KeywordParameters,
            "other args" | "other arg" | "other arguments" => Self::OtherParameters,
            "other params" | "other param" | "other parameters" => Self::OtherParameters,
            "receives" | "receive" => Self::Receives,
            "returns" | "return" => Self::Returns,
            "yields" | "yield" => Self::Yields,
            "raises" | "raise" => Self::Raises,
            "warns" | "warn" => Self::Warns,
            "see also" => Self::SeeAlso,
            "attributes" | "attribute" => Self::Attributes,
            "methods" | "method" => Self::Methods,
            "notes" | "note" => Self::Notes,
            "examples" | "example" => Self::Examples,
            "todo" => Self::Todo,
            "references" | "reference" => Self::References,
            "warnings" | "warning" => Self::Warnings,
            "attention" => Self::Attention,
            "caution" => Self::Caution,
            "danger" => Self::Danger,
            "error" => Self::Error,
            "hint" => Self::Hint,
            "important" => Self::Important,
            "tip" => Self::Tip,
            _ => Self::Unknown,
        }
    }

    /// Convert a **lowercased** section name using the Google alias table.
    #[rustfmt::skip]
    pub(crate) fn from_google_name(name: &str) -> Self {
        match name {
            // Google-only: historic typo aliases (snapshot-pinned).
            "argment" | "paramter" => Self::Parameters,
            "keyword paramter" => Self::KeywordParameters,
            "other argment" | "other paramter" => Self::OtherParameters,
            _ => Self::from_common_name(name),
        }
    }

    /// Convert a **lowercased** section name using the NumPy alias table.
    #[rustfmt::skip]
    pub(crate) fn from_numpy_name(name: &str) -> Self {
        match name {
            // NumPy-only: singular full forms (snapshot-pinned).
            "parameter" | "argument" => Self::Parameters,
            "keyword parameter" => Self::KeywordParameters,
            "other parameter" | "other argument" => Self::OtherParameters,
            _ => Self::from_common_name(name),
        }
    }

    /// Whether the Google alias table knows this lowercased name.
    pub(crate) fn is_known_google(name: &str) -> bool {
        !matches!(Self::from_google_name(name), Self::Unknown)
    }

    /// Whether the NumPy alias table knows this lowercased name.
    pub(crate) fn is_known_numpy(name: &str) -> bool {
        !matches!(Self::from_numpy_name(name), Self::Unknown)
    }

    /// The [`EntryRole`] of this section's body entries.
    ///
    /// Shared by the visitor's `ENTRY` routing and the typed section
    /// accessors' role guards.
    pub(crate) fn entry_role(self) -> EntryRole {
        match self {
            Self::Parameters | Self::KeywordParameters | Self::OtherParameters | Self::Receives => EntryRole::Parameter,
            Self::Returns => EntryRole::Return,
            Self::Yields => EntryRole::Yield,
            Self::Raises => EntryRole::Exception,
            Self::Warns => EntryRole::Warning,
            Self::SeeAlso => EntryRole::SeeAlsoItem,
            Self::Attributes => EntryRole::Attribute,
            Self::Methods => EntryRole::Method,
            Self::References => EntryRole::Citation,
            // Notes, Examples, Todo, Warnings, admonitions, Unknown, and any
            // future kinds: free-text body, no entries.
            _ => EntryRole::FreeText,
        }
    }

    /// Map to the style-independent [`SectionKind`] of the model layer.
    ///
    /// `header_name` is the section header text as written, used for
    /// [`FreeSectionKind::Unknown`].
    #[rustfmt::skip]
    pub(crate) fn to_section_kind(self, header_name: &str) -> SectionKind {
        match self {
            Self::Parameters => SectionKind::Parameters,
            Self::KeywordParameters => SectionKind::KeywordParameters,
            Self::OtherParameters => SectionKind::OtherParameters,
            Self::Receives => SectionKind::Receives,
            Self::Returns => SectionKind::Returns,
            Self::Yields => SectionKind::Yields,
            Self::Raises => SectionKind::Raises,
            Self::Warns => SectionKind::Warns,
            Self::Attributes => SectionKind::Attributes,
            Self::Methods => SectionKind::Methods,
            Self::SeeAlso => SectionKind::SeeAlso,
            Self::References => SectionKind::References,
            Self::Notes => SectionKind::FreeText(FreeSectionKind::Notes),
            Self::Examples => SectionKind::FreeText(FreeSectionKind::Examples),
            Self::Todo => SectionKind::FreeText(FreeSectionKind::Todo),
            Self::Warnings => SectionKind::FreeText(FreeSectionKind::Warnings),
            Self::Attention => SectionKind::FreeText(FreeSectionKind::Attention),
            Self::Caution => SectionKind::FreeText(FreeSectionKind::Caution),
            Self::Danger => SectionKind::FreeText(FreeSectionKind::Danger),
            Self::Error => SectionKind::FreeText(FreeSectionKind::Error),
            Self::Hint => SectionKind::FreeText(FreeSectionKind::Hint),
            Self::Important => SectionKind::FreeText(FreeSectionKind::Important),
            Self::Tip => SectionKind::FreeText(FreeSectionKind::Tip),
            Self::Unknown => SectionKind::FreeText(FreeSectionKind::Unknown(header_name.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-style tables differ exactly by the documented deltas: Google's
    /// typo aliases and NumPy's singular full forms. Everything else resolves
    /// identically.
    #[test]
    fn alias_tables_differ_only_by_the_documented_deltas() {
        for (google_only, kind) in [
            ("argment", SectionName::Parameters),
            ("paramter", SectionName::Parameters),
            ("keyword paramter", SectionName::KeywordParameters),
            ("other argment", SectionName::OtherParameters),
            ("other paramter", SectionName::OtherParameters),
        ] {
            assert_eq!(SectionName::from_google_name(google_only), kind);
            assert_eq!(SectionName::from_numpy_name(google_only), SectionName::Unknown);
        }
        for (numpy_only, kind) in [
            ("parameter", SectionName::Parameters),
            ("argument", SectionName::Parameters),
            ("keyword parameter", SectionName::KeywordParameters),
            ("other parameter", SectionName::OtherParameters),
            ("other argument", SectionName::OtherParameters),
        ] {
            assert_eq!(SectionName::from_numpy_name(numpy_only), kind);
            assert_eq!(SectionName::from_google_name(numpy_only), SectionName::Unknown);
        }
        for shared in [
            "args",
            "parameters",
            "returns",
            "see also",
            "keyword argument",
            "other params",
        ] {
            assert_eq!(
                SectionName::from_google_name(shared),
                SectionName::from_numpy_name(shared),
                "shared alias {shared:?} diverged between the tables"
            );
            assert_ne!(SectionName::from_google_name(shared), SectionName::Unknown);
        }
    }
}
