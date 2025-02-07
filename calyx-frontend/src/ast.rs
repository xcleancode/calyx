//! Abstract Syntax Tree for Calyx
use super::parser;
use crate::{Attributes, PortDef, Primitive};
use atty::Stream;
use calyx_utils::{CalyxResult, Error, GPosIdx, Id};
use std::path::PathBuf;

/// Corresponds to an individual Calyx file.
#[derive(Debug)]
pub struct NamespaceDef {
    /// Path to extern files.
    pub imports: Vec<String>,
    /// List of component definitions.
    pub components: Vec<ComponentDef>,
    /// Extern statements and any primitive declarations in them.
    pub externs: Vec<(Option<String>, Vec<Primitive>)>,
    /// Optional opaque metadata
    pub metadata: Option<String>,
}

impl NamespaceDef {
    pub fn construct(file: &Option<PathBuf>) -> CalyxResult<Self> {
        match file {
            Some(file) => parser::CalyxParser::parse_file(file),
            None => {
                if atty::isnt(Stream::Stdin) {
                    parser::CalyxParser::parse(std::io::stdin())
                } else {
                    Err(Error::invalid_file(
                        "No file provided and terminal not a TTY".to_string(),
                    ))
                }
            }
        }
    }
}

/// AST statement for defining components.
#[derive(Debug)]
pub struct ComponentDef {
    /// Name of the component.
    pub name: Id,
    /// Defines input and output ports along with their attributes.
    pub signature: Vec<PortDef<u64>>,
    /// List of instantiated sub-components
    pub cells: Vec<Cell>,
    /// List of groups
    pub groups: Vec<Group>,
    /// List of continuous assignments
    pub continuous_assignments: Vec<Wire>,
    /// Single control statement for this component.
    pub control: Control,
    /// Attributes attached to this component
    pub attributes: Attributes,
    /// True iff this is a combinational component
    pub is_comb: bool,
}

impl ComponentDef {
    pub fn new<S>(name: S, is_comb: bool, signature: Vec<PortDef<u64>>) -> Self
    where
        S: Into<Id>,
    {
        Self {
            name: name.into(),
            signature,
            cells: Vec::new(),
            groups: Vec::new(),
            continuous_assignments: Vec::new(),
            control: Control::empty(),
            attributes: Attributes::default(),
            is_comb,
        }
    }
}

/// Statement that refers to a port on a subcomponent.
/// This is distinct from a `Portdef` which defines a port.
#[derive(Debug)]
pub enum Port {
    /// Refers to the port named `port` on the subcomponent
    /// `component`.
    Comp { component: Id, port: Id },

    /// Refers to the port named `port` on the component
    /// currently being defined.
    This { port: Id },

    /// `group[name]` parses into `Hole { group, name }`
    /// and is a hole named `name` on group `group`
    Hole { group: Id, name: Id },
}

// ===================================
// AST for wire guard expressions
// ===================================

#[derive(Debug)]
pub enum NumType {
    Decimal,
    Binary,
    Octal,
    Hex,
}

/// Custom bitwidth numbers
#[derive(Debug)]
pub struct BitNum {
    pub width: u64,
    pub num_type: NumType,
    pub val: u64,
    pub span: GPosIdx,
}

/// Atomic operations used in guard conditions and RHS of the
/// guarded assignments.
#[derive(Debug)]
pub enum Atom {
    /// Accessing a particular port on a component.
    Port(Port),
    /// A constant.
    Num(BitNum),
}

/// The AST for GuardExprs
#[derive(Debug)]
pub enum GuardExpr {
    // Logical operations
    And(Box<GuardExpr>, Box<GuardExpr>),
    Or(Box<GuardExpr>, Box<GuardExpr>),
    Not(Box<GuardExpr>),
    CompOp(GuardComp, Atom, Atom),
    Atom(Atom),
}

/// Possible comparison operators for guards.
#[derive(Debug)]
pub enum GuardComp {
    Eq,
    Neq,
    Gt,
    Lt,
    Geq,
    Leq,
}

/// Guards `expr` using the optional guard condition `guard`.
#[derive(Debug)]
pub struct Guard {
    pub guard: Option<GuardExpr>,
    pub expr: Atom,
}

// ===================================
// Data definitions for Structure
// ===================================

/// Prototype of the cell definition
#[derive(Debug)]
pub struct Proto {
    /// Name of the primitive.
    pub name: Id,
    /// Parameter binding for primitives
    pub params: Vec<u64>,
}

/// The Cell AST nodes.
#[derive(Debug)]
pub struct Cell {
    /// Name of the cell.
    pub name: Id,
    /// Name of the prototype this cell was built from.
    pub prototype: Proto,
    /// Attributes attached to this cell definition
    pub attributes: Attributes,
    /// Whether this cell is external
    pub reference: bool,
}

/// Methods for constructing the structure AST nodes.
impl Cell {
    /// Constructs a primitive cell instantiation.
    pub fn from(
        name: Id,
        proto: Id,
        params: Vec<u64>,
        attributes: Attributes,
        reference: bool,
    ) -> Cell {
        Cell {
            name,
            prototype: Proto {
                name: proto,
                params,
            },
            attributes,
            reference,
        }
    }
}

#[derive(Debug)]
pub struct Group {
    pub name: Id,
    pub wires: Vec<Wire>,
    pub attributes: Attributes,
    pub is_comb: bool,
}

/// Data for the `->` structure statement.
#[derive(Debug)]
pub struct Wire {
    /// Source of the wire.
    pub src: Guard,

    /// Guarded destinations of the wire.
    pub dest: Port,

    /// Attributes for this assignment
    pub attributes: Attributes,
}

/// Control AST nodes.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Control {
    /// Represents sequential composition of control statements.
    Seq {
        /// List of `Control` statements to run in sequence.
        stmts: Vec<Control>,
        /// Attributes
        attributes: Attributes,
    },
    /// Represents parallel composition of control statements.
    Par {
        /// List of `Control` statements to run in sequence.
        stmts: Vec<Control>,
        /// Attributes
        attributes: Attributes,
    },
    /// Standard imperative if statement
    If {
        /// Port that connects the conditional check.
        port: Port,

        /// Modules that need to be enabled to send signal on `port`.
        cond: Option<Id>,

        /// Control for the true branch.
        tbranch: Box<Control>,

        /// Control for the true branch.
        fbranch: Box<Control>,

        /// Attributes
        attributes: Attributes,
    },
    /// Standard imperative while statement
    While {
        /// Port that connects the conditional check.
        port: Port,

        /// Modules that need to be enabled to send signal on `port`.
        cond: Option<Id>,

        /// Control for the loop body.
        body: Box<Control>,

        /// Attributes
        attributes: Attributes,
    },
    /// Runs the control for a list of subcomponents.
    Enable {
        /// Group to be enabled
        comp: Id,
        /// Attributes
        attributes: Attributes,
    },
    /// Invoke component with input/output assignments.
    Invoke {
        /// Name of the component to be invoked.
        comp: Id,
        /// Input assignments
        inputs: Vec<(Id, Atom)>,
        /// Output assignments
        outputs: Vec<(Id, Atom)>,
        /// Attributes
        attributes: Attributes,
        /// Combinational group that may execute with this invoke.
        comb_group: Option<Id>,
        /// External cells that may execute with this invoke.
        ref_cells: Vec<(Id, Id)>,
    },
    /// Control statement that does nothing.
    Empty {
        /// Attributes
        attributes: Attributes,
    },
}

impl Control {
    pub fn empty() -> Control {
        Control::Empty {
            attributes: Attributes::default(),
        }
    }
}
