//! NumPy-style section kind enumeration.

use core::fmt;

/// NumPy-style section kinds.
///
/// Each variant represents a recognised section name (or group of aliases),
/// or [`Unknown`](Self::Unknown) for unrecognised names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NumPySectionKind {
    /// `Parameters` / `Params`
    Parameters,
    /// `Returns` / `Return`
    Returns,
    /// `Yields` / `Yield`
    Yields,
    /// `Receives` / `Receive`
    Receives,
    /// `Other Parameters` / `Other Params`
    OtherParameters,
    /// `Keyword Parameters` / `Keyword Arguments` / `Keyword Args`
    KeywordParameters,
    /// `Raises` / `Raise`
    Raises,
    /// `Warns` / `Warn`
    Warns,
    /// `Warnings` / `Warning`
    Warnings,
    /// `See Also`
    SeeAlso,
    /// `Notes` / `Note`
    Notes,
    /// `References`
    References,
    /// `Examples` / `Example`
    Examples,
    /// `Attributes`
    Attributes,
    /// `Methods`
    Methods,
    /// `Todo`
    Todo,
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

impl NumPySectionKind {
    /// The [`EntryRole`](crate::parse::EntryRole) of this section's body
    /// entries.
    ///
    /// Shared by the visitor's `ENTRY` routing and the typed section
    /// accessors' role guards.
    pub(crate) fn entry_role(self) -> crate::parse::EntryRole {
        use crate::parse::EntryRole;
        match self {
            Self::Parameters | Self::OtherParameters | Self::Receives | Self::KeywordParameters => EntryRole::Parameter,
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

    /// All known section kinds.
    pub const ALL: &[NumPySectionKind] = &[
        Self::Parameters,
        Self::Returns,
        Self::Yields,
        Self::Receives,
        Self::OtherParameters,
        Self::KeywordParameters,
        Self::Raises,
        Self::Warns,
        Self::Warnings,
        Self::SeeAlso,
        Self::Notes,
        Self::References,
        Self::Examples,
        Self::Attributes,
        Self::Methods,
        Self::Todo,
        Self::Attention,
        Self::Caution,
        Self::Danger,
        Self::Error,
        Self::Hint,
        Self::Important,
        Self::Tip,
    ];

    /// Convert a **lowercased** section name to a [`NumPySectionKind`].
    #[rustfmt::skip]
    pub fn from_name(name: &str) -> Self {
        match name {
            "parameters" | "parameter" | "params" | "param" => Self::Parameters,
            "arguments" | "argument" | "args" | "arg" => Self::Parameters,
            "returns" | "return" => Self::Returns,
            "yields" | "yield" => Self::Yields,
            "receives" | "receive" => Self::Receives,
            "other parameters" | "other parameter" | "other params" | "other param" => Self::OtherParameters,
            "other arguments" | "other argument" | "other args" | "other arg" => Self::OtherParameters,
            "keyword parameters" | "keyword parameter" | "keyword params" | "keyword param" => Self::KeywordParameters,
            "keyword arguments" | "keyword argument" | "keyword args" | "keyword arg" => Self::KeywordParameters,
            "raises" | "raise" => Self::Raises,
            "warns" | "warn" => Self::Warns,
            "warnings" | "warning" => Self::Warnings,
            "see also" => Self::SeeAlso,
            "notes" | "note" => Self::Notes,
            "references" | "reference" => Self::References,
            "examples" | "example" => Self::Examples,
            "attributes" | "attribute" => Self::Attributes,
            "methods" | "method" => Self::Methods,
            "todo" => Self::Todo,
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

    /// Whether this section kind has structured entries (vs free text).
    pub fn is_structured(&self) -> bool {
        matches!(
            self,
            Self::Parameters
                | Self::Returns
                | Self::Yields
                | Self::Receives
                | Self::OtherParameters
                | Self::KeywordParameters
                | Self::Raises
                | Self::Warns
                | Self::SeeAlso
                | Self::References
                | Self::Attributes
                | Self::Methods
        )
    }

    /// Whether this section kind has free-text body.
    pub fn is_freetext(&self) -> bool {
        matches!(
            self,
            Self::Notes
                | Self::Examples
                | Self::Warnings
                | Self::Todo
                | Self::Attention
                | Self::Caution
                | Self::Danger
                | Self::Error
                | Self::Hint
                | Self::Important
                | Self::Tip
                | Self::Unknown
        )
    }
}

impl fmt::Display for NumPySectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Parameters => "Parameters",
            Self::Returns => "Returns",
            Self::Yields => "Yields",
            Self::Receives => "Receives",
            Self::OtherParameters => "Other Parameters",
            Self::KeywordParameters => "Keyword Parameters",
            Self::Raises => "Raises",
            Self::Warns => "Warns",
            Self::Warnings => "Warnings",
            Self::SeeAlso => "See Also",
            Self::Notes => "Notes",
            Self::References => "References",
            Self::Examples => "Examples",
            Self::Attributes => "Attributes",
            Self::Methods => "Methods",
            Self::Todo => "Todo",
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
