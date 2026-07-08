//! Google-style section kind enumeration.

use core::fmt;

use crate::model::FreeSectionKind;
use crate::model::SectionKind;

/// Google-style section kinds.
///
/// Each variant represents a recognised section name (or group of aliases),
/// or [`Unknown`](Self::Unknown) for unrecognised names.
/// Use [`GoogleSectionKind::from_name`] to convert a lowercased section name
/// to a variant.
///
/// Having an enum instead of a plain string list gives compile-time
/// exhaustiveness checks: every variant must be handled when matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GoogleSectionKind {
    /// `Args` / `Arguments` / `Parameters` / `Params`
    Args,
    /// `Keyword Args` / `Keyword Arguments`
    KeywordArgs,
    /// `Other Parameters`
    OtherParameters,
    /// `Receive` / `Receives`
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
    /// `Methods`
    Methods,
    /// `See Also`
    SeeAlso,
    /// `Note` / `Notes`
    Notes,
    /// `Example` / `Examples`
    Examples,
    /// `Todo`
    Todo,
    /// `References`
    References,
    /// `Warning` / `Warnings`
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

impl GoogleSectionKind {
    /// The [`EntryRole`](crate::parse::EntryRole) of this section's body
    /// entries.
    ///
    /// Shared by the visitor's `ENTRY` routing and the typed section
    /// accessors' role guards.
    pub(crate) fn entry_role(self) -> crate::parse::EntryRole {
        use crate::parse::EntryRole;
        match self {
            Self::Args | Self::KeywordArgs | Self::OtherParameters | Self::Receives => EntryRole::Parameter,
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

    /// All known section kinds (useful for iteration / testing).
    pub const ALL: &[GoogleSectionKind] = &[
        Self::Args,
        Self::KeywordArgs,
        Self::OtherParameters,
        Self::Receives,
        Self::Returns,
        Self::Yields,
        Self::Raises,
        Self::Warns,
        Self::Attributes,
        Self::Methods,
        Self::SeeAlso,
        Self::Notes,
        Self::Examples,
        Self::Todo,
        Self::References,
        Self::Warnings,
        Self::Attention,
        Self::Caution,
        Self::Danger,
        Self::Error,
        Self::Hint,
        Self::Important,
        Self::Tip,
    ];

    /// Convert a **lowercased** section name to a [`GoogleSectionKind`].
    ///
    /// Returns [`Unknown`](Self::Unknown) for unrecognised names.
    #[rustfmt::skip]
    pub fn from_name(name: &str) -> Self {
        match name {
            "args" | "arg" | "arguments" | "argment" => Self::Args,
            "params" | "param" | "parameters" | "paramter" => Self::Args,
            "keyword args" | "keyword arg" | "keyword arguments" | "keyword argument" => Self::KeywordArgs,
            "keyword params" | "keyword param" | "keyword parameters" | "keyword paramter" => Self::KeywordArgs,
            "other args" | "other arg" | "other arguments" | "other argment" => Self::OtherParameters,
            "other params" | "other param" | "other parameters" | "other paramter" => Self::OtherParameters,
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

    /// Check if a lowercased name is a known (non-[`Unknown`](Self::Unknown)) section name.
    pub fn is_known(name: &str) -> bool {
        !matches!(Self::from_name(name), Self::Unknown)
    }

    /// Map to the style-independent [`SectionKind`] of the model layer.
    ///
    /// `header_name` is the section header text as written, used for
    /// [`FreeSectionKind::Unknown`].
    #[rustfmt::skip]
    pub fn to_section_kind(self, header_name: &str) -> SectionKind {
        match self {
            Self::Args => SectionKind::Parameters,
            Self::KeywordArgs => SectionKind::KeywordParameters,
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

    /// Whether this section kind uses structured (entry-based) body parsing.
    pub fn is_structured(self) -> bool {
        matches!(
            self,
            Self::Args
                | Self::KeywordArgs
                | Self::OtherParameters
                | Self::Receives
                | Self::Returns
                | Self::Yields
                | Self::Raises
                | Self::Warns
                | Self::Attributes
                | Self::Methods
                | Self::SeeAlso
                | Self::References
        )
    }

    /// Whether this section kind uses free-text body parsing.
    pub fn is_freetext(self) -> bool {
        !self.is_structured()
    }
}

impl fmt::Display for GoogleSectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Args => "Args",
            Self::KeywordArgs => "Keyword Args",
            Self::OtherParameters => "Other Parameters",
            Self::Receives => "Receives",
            Self::Returns => "Returns",
            Self::Yields => "Yields",
            Self::Raises => "Raises",
            Self::Warns => "Warns",
            Self::SeeAlso => "See Also",
            Self::Attributes => "Attributes",
            Self::Methods => "Methods",
            Self::Notes => "Notes",
            Self::Examples => "Examples",
            Self::Todo => "Todo",
            Self::References => "References",
            Self::Warnings => "Warnings",
            Self::Attention => "Attention",
            Self::Caution => "Caution",
            Self::Danger => "Danger",
            Self::Error => "Error",
            Self::Hint => "Hint",
            Self::Important => "Important",
            Self::Tip => "Tip",
            Self::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}
