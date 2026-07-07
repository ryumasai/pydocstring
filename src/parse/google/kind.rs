//! Google-style section kind enumeration.

use core::fmt;

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
