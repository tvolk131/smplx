use std::sync::Arc;

use simplicityhl::str::Identifier;
use simplicityhl::{
    ResolvedType, Value,
    either::Either,
    types::{TypeConstructible, TypeInner},
    value::{ValueConstructible, ValueInner},
};

use crate::signer::error::WtnsWrappingError;

/// Represents an index-based route for array or tuple witness types.
#[derive(Clone, Copy, Debug)]
pub(super) struct EnumerableRoute(usize);

/// Represents a branch route for `Either` witness type.
#[derive(Clone, Copy, Debug)]
pub(super) enum EitherRoute {
    Left,
    Right,
}

/// Represents a single step in a witness path: either a name (`Left`, `Right`,
/// or an enum variant name, interpreted against the current type) or an index.
#[derive(Clone, Debug)]
pub(super) enum WtnsPathRoute {
    Name(String),
    Index(usize),
}

/// Exposes utilities to safely inject values into specific locations within a Simplicity witness structure.
#[derive(Clone)]
pub(super) struct WtnsInjector {}

enum StackItem {
    Either(EitherRoute, Arc<ResolvedType>),
    Array(EnumerableRoute, Arc<ResolvedType>, Arc<[Value]>),
    Tuple(EnumerableRoute, Arc<[Value]>),
    Enum {
        variant_name: String,
        enum_ty: Arc<ResolvedType>,
        /// Single-payload variants rebuild from the payload value directly;
        /// multi-payload variants rebuild from a synthesized tuple of payloads.
        payload_len: usize,
    },
}

impl WtnsInjector {
    /// Constructs a new value by injecting a given value into the witness at the position described by `path`.
    ///
    /// Consistency between `witness` and `witness_types` should be guaranteed by the caller.
    ///
    /// # Errors
    /// Returns a `WtnsWrappingError` if the path contains invalid segments, attempts to access an out-of-bounds index, navigates into an incorrect type layout, or expects a different branch or enum variant representation.
    ///
    /// # Panics
    /// Panics if internal type validations or downcasts fail after safety checks have passed.
    #[allow(clippy::too_many_lines)]
    pub(super) fn inject_value<I>(
        witness: &Arc<Value>,
        witness_types: &ResolvedType,
        path: I,
        value: Value,
    ) -> Result<Value, WtnsWrappingError>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let parsed_path = Self::parse_path(path);

        let mut stack = Vec::new();
        let mut current_val = Arc::clone(witness);
        let mut current_ty = witness_types.clone();

        for route in &parsed_path {
            let route_supported = match (route, current_ty.as_inner()) {
                (WtnsPathRoute::Index(_), TypeInner::Array(_, _) | TypeInner::Tuple(_))
                | (WtnsPathRoute::Name(_), TypeInner::Enum(_)) => true,
                (WtnsPathRoute::Name(name), TypeInner::Either(_, _)) => matches!(name.as_str(), "Left" | "Right"),
                _ => false,
            };
            if !route_supported {
                return Err(WtnsWrappingError::UnsupportedPathType(current_ty.to_string()));
            }

            match current_ty.as_inner() {
                TypeInner::Either(left_ty, right_ty) => {
                    let WtnsPathRoute::Name(name) = route else {
                        unreachable!("checked in route_supported above");
                    };
                    let direction = match name.as_str() {
                        "Left" => EitherRoute::Left,
                        "Right" => EitherRoute::Right,
                        _ => unreachable!("checked in route_supported above"),
                    };
                    let either_val = Self::downcast_either(&current_val);

                    match (direction, either_val.is_right()) {
                        (EitherRoute::Left, false) => {
                            stack.push(StackItem::Either(direction, Arc::clone(right_ty)));
                            current_ty = left_ty.as_ref().clone();
                            current_val = Arc::clone(either_val.as_ref().unwrap_left());
                        }
                        (EitherRoute::Right, true) => {
                            stack.push(StackItem::Either(direction, Arc::clone(left_ty)));
                            current_ty = right_ty.as_ref().clone();
                            current_val = Arc::clone(either_val.as_ref().unwrap_right());
                        }
                        _ => return Err(WtnsWrappingError::EitherBranchMismatch),
                    }
                }
                TypeInner::Array(ty, len) => {
                    let WtnsPathRoute::Index(idx) = route else {
                        unreachable!("checked in route_supported above");
                    };

                    if *idx >= *len {
                        return Err(WtnsWrappingError::IdxOutOfBounds(*len, *idx));
                    }
                    let idx = EnumerableRoute(*idx);

                    let arr_val = Self::downcast_array(&current_val);

                    stack.push(StackItem::Array(idx, Arc::clone(ty), Arc::clone(&arr_val)));

                    current_ty = ty.as_ref().clone();
                    current_val = Arc::new(arr_val[idx.0].clone());
                }
                TypeInner::Tuple(tuple) => {
                    let WtnsPathRoute::Index(idx) = route else {
                        unreachable!("checked in route_supported above");
                    };

                    if *idx >= tuple.len() {
                        return Err(WtnsWrappingError::IdxOutOfBounds(tuple.len(), *idx));
                    }
                    let idx = EnumerableRoute(*idx);

                    let tuple_val = Self::downcast_tuple(&current_val);

                    stack.push(StackItem::Tuple(idx, Arc::clone(&tuple_val)));

                    current_ty = tuple[idx.0].as_ref().clone();
                    current_val = Arc::new(tuple_val[idx.0].clone());
                }
                TypeInner::Enum(info) => {
                    let WtnsPathRoute::Name(name) = route else {
                        unreachable!("checked in route_supported above");
                    };
                    let Some((variant_index, variant)) = info.variant(&Identifier::from_str_unchecked(name)) else {
                        return Err(WtnsWrappingError::UnknownEnumVariant(
                            name.clone(),
                            current_ty.to_string(),
                        ));
                    };

                    let (value_variant_index, payload) = match current_val.inner() {
                        ValueInner::Enum(index, payload) => (*index, Arc::clone(payload)),
                        _ => unreachable!("value is type-checked against witness types"),
                    };
                    if value_variant_index != variant_index {
                        let held_variant = info.variants()[value_variant_index].name().to_string();
                        return Err(WtnsWrappingError::EnumVariantMismatch(name.clone(), held_variant));
                    }

                    stack.push(StackItem::Enum {
                        variant_name: name.clone(),
                        enum_ty: Arc::new(current_ty.clone()),
                        payload_len: variant.payload().len(),
                    });

                    match variant.payload().len() {
                        0 => {
                            return Err(WtnsWrappingError::UnsupportedPathType(current_ty.to_string()));
                        }
                        1 => {
                            current_ty = variant.payload()[0].clone();
                            current_val = Arc::new(payload[0].clone());
                        }
                        _ => {
                            // Synthesize a tuple view over the payload so that
                            // subsequent index routes address payload elements.
                            current_ty = ResolvedType::tuple(variant.payload().iter().cloned());
                            current_val = Arc::new(Value::tuple(payload.to_vec()));
                        }
                    }
                }
                _ => unreachable!("checked in route_supported above"),
            }
        }

        if value.ty() != &current_ty {
            return Err(WtnsWrappingError::RootTypeMismatch(
                current_ty.to_string(),
                value.ty().to_string(),
            ));
        }

        let mut value = value;

        for item in stack.into_iter().rev() {
            value = match item {
                StackItem::Either(direction, sibling_ty) => match direction {
                    EitherRoute::Left => Value::left(value, (*sibling_ty).clone()),
                    EitherRoute::Right => Value::right((*sibling_ty).clone(), value),
                },
                StackItem::Array(idx, elem_ty, arr) => {
                    let mut elements = arr.to_vec();
                    elements[idx.0] = value;
                    Value::array(elements, (*elem_ty).clone())
                }
                StackItem::Tuple(idx, tuple_vals) => {
                    let mut elements = tuple_vals.to_vec();
                    elements[idx.0] = value;
                    Value::tuple(elements)
                }
                StackItem::Enum {
                    variant_name,
                    enum_ty,
                    payload_len,
                } => {
                    let payload = if payload_len == 1 {
                        vec![value]
                    } else {
                        match value.inner() {
                            ValueInner::Tuple(elements) => elements.to_vec(),
                            _ => unreachable!("multi-payload variants rebuild through a tuple view"),
                        }
                    };

                    Value::enum_variant(&enum_ty, &Identifier::from_str_unchecked(&variant_name), payload)
                        .expect("variant and payload are type-checked during traversal")
                }
            };
        }

        Ok(value)
    }

    fn parse_path<I>(path: I) -> Vec<WtnsPathRoute>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        path.into_iter()
            .map(|route| match route.as_ref().parse::<usize>() {
                Ok(index) => WtnsPathRoute::Index(index),
                Err(_) => WtnsPathRoute::Name(route.as_ref().to_string()),
            })
            .collect()
    }

    fn downcast_either(val: &Value) -> &Either<Arc<Value>, Arc<Value>> {
        match val.inner() {
            ValueInner::Either(either) => either,
            _ => unreachable!(),
        }
    }

    fn downcast_array(val: &Value) -> Arc<[Value]> {
        match val.inner() {
            ValueInner::Array(arr) => Arc::clone(arr),
            _ => unreachable!(),
        }
    }

    fn downcast_tuple(val: &Value) -> Arc<[Value]> {
        match val.inner() {
            ValueInner::Tuple(arr) => Arc::clone(arr),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod test {
    use simplicityhl::ast::ElementsJetHinter;
    use simplicityhl::str::Identifier;
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::{TemplateProgram, UnstableFeatures};

    use super::*;

    fn dummy_value() -> Value {
        // Either<(u64, Either<u64, u64>), [u8; 4]>
        Value::left(
            Value::tuple([Value::u64(0), Value::right(ResolvedType::u64(), Value::u64(1))]),
            ResolvedType::array(ResolvedType::u8(), 64),
        )
    }

    fn witness_type(source: &str) -> ResolvedType {
        let template = TemplateProgram::new_with_unstable(
            Arc::from(source),
            &UnstableFeatures::all(),
            Box::new(ElementsJetHinter),
        )
        .unwrap();

        let (_, ty) = template.witness_types().iter().next().unwrap();
        ty.clone()
    }

    fn enum_type() -> ResolvedType {
        witness_type(
            "
enum Mode {
    Off,
    Single(u32),
    Pair(u32, u64),
}

fn main() {
    let mode: Mode = witness::MODE;
}
",
        )
    }

    #[test]
    fn inject_value_success() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let injected_val_tuple =
            WtnsInjector::inject_value(&witness, witness_types, &["Left", "0"], Value::u64(3)).unwrap();

        assert_eq!(
            injected_val_tuple,
            Value::parse_from_str("Left((3, Right(1)))", witness_types).unwrap()
        );

        let injected_val_either = WtnsInjector::inject_value(
            &witness,
            witness_types,
            &["Left", "1"],
            Value::left(Value::u64(2), ResolvedType::u64()),
        )
        .unwrap();

        assert_eq!(
            injected_val_either,
            Value::parse_from_str("Left((0, Left(2)))", witness_types).unwrap()
        );
    }

    #[test]
    fn inject_value_idx_out_of_bounds() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let err = WtnsInjector::inject_value(&witness, witness_types, &["Left", "5"], Value::u64(0)).unwrap_err();

        assert!(matches!(err, WtnsWrappingError::IdxOutOfBounds(_, _)));
    }

    #[test]
    fn inject_value_root_mismatch() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let err = WtnsInjector::inject_value(&witness, witness_types, &["Left", "1"], Value::unit()).unwrap_err();

        assert!(matches!(err, WtnsWrappingError::RootTypeMismatch(_, _)));
    }

    #[test]
    fn inject_value_either_branch_mismatch() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let err = WtnsInjector::inject_value(
            &witness,
            witness_types,
            &["Right"],
            Value::right(
                ResolvedType::tuple([
                    ResolvedType::u64(),
                    ResolvedType::either(ResolvedType::u64(), ResolvedType::u64()),
                ]),
                Value::array(vec![Value::u8(0)], ResolvedType::u8()),
            ),
        )
        .unwrap_err();

        assert!(matches!(err, WtnsWrappingError::EitherBranchMismatch));
    }

    fn enum_variant(ty: &ResolvedType, name: &str, payload: Vec<Value>) -> Value {
        Value::enum_variant(ty, &Identifier::from_str_unchecked(name), payload).unwrap()
    }

    #[test]
    fn inject_value_enum_single_payload() {
        let enum_ty = enum_type();
        let witness = Arc::new(enum_variant(&enum_ty, "Single", vec![Value::u32(0)]));

        let injected = WtnsInjector::inject_value(&witness, &enum_ty, &["Single"], Value::u32(9)).unwrap();

        assert_eq!(injected, enum_variant(&enum_ty, "Single", vec![Value::u32(9)]));
    }

    #[test]
    fn inject_value_enum_multi_payload() {
        let enum_ty = enum_type();
        let witness = Arc::new(enum_variant(&enum_ty, "Pair", vec![Value::u32(1), Value::u64(2)]));

        let injected = WtnsInjector::inject_value(&witness, &enum_ty, &["Pair", "1"], Value::u64(9)).unwrap();

        assert_eq!(
            injected,
            enum_variant(&enum_ty, "Pair", vec![Value::u32(1), Value::u64(9)])
        );
    }

    #[test]
    fn inject_value_enum_unknown_variant() {
        let enum_ty = enum_type();
        let witness = Arc::new(enum_variant(&enum_ty, "Single", vec![Value::u32(0)]));

        let err = WtnsInjector::inject_value(&witness, &enum_ty, &["Nope"], Value::u32(9)).unwrap_err();

        assert!(matches!(err, WtnsWrappingError::UnknownEnumVariant(name, _) if name == "Nope"));
    }

    #[test]
    fn inject_value_enum_variant_mismatch() {
        let enum_ty = enum_type();
        let witness = Arc::new(enum_variant(&enum_ty, "Single", vec![Value::u32(0)]));

        let err = WtnsInjector::inject_value(&witness, &enum_ty, &["Pair", "0"], Value::u32(9)).unwrap_err();

        assert!(
            matches!(err, WtnsWrappingError::EnumVariantMismatch(selected, held) if selected == "Pair" && held == "Single")
        );
    }

    #[test]
    fn inject_value_enum_unit_variant_unsupported() {
        let enum_ty = enum_type();
        let witness = Arc::new(enum_variant(&enum_ty, "Off", vec![]));

        let err = WtnsInjector::inject_value(&witness, &enum_ty, &["Off"], Value::unit()).unwrap_err();

        assert!(matches!(err, WtnsWrappingError::UnsupportedPathType(_)));
    }

    fn nested_enum_type() -> ResolvedType {
        witness_type(
            "
enum Mode {
    Off,
    Single(u32),
}

enum Wallet {
    Left(u32),
    Right(u64),
}

enum Deep {
    Empty,
    Inner(Either<Mode, Wallet>),
    Bag([Mode; 2]),
}

fn main() {
    let nested: Deep = witness::NESTED;
}
",
        )
    }

    /// The payload types of `Deep::Inner(Either<Mode, Wallet>)`.
    fn inner_either_types(deep_ty: &ResolvedType) -> (ResolvedType, ResolvedType) {
        let info = deep_ty.as_enum().expect("Deep is an enum");
        let (_, variant) = info.variant(&Identifier::from_str_unchecked("Inner")).expect("variant");
        match variant.payload()[0].as_inner() {
            TypeInner::Either(left, right) => (left.as_ref().clone(), right.as_ref().clone()),
            _ => unreachable!("Inner payload is an Either"),
        }
    }

    #[test]
    fn inject_value_nested_through_enum_and_either() {
        let deep_ty = nested_enum_type();
        let (mode_ty, _) = inner_either_types(&deep_ty);

        let witness = Arc::new(Value::parse_from_str("Deep::Inner(Left(Mode::Off))", &deep_ty).unwrap());

        let injected = WtnsInjector::inject_value(
            &witness,
            &deep_ty,
            &["Inner", "Left"],
            enum_variant(&mode_ty, "Single", vec![Value::u32(9)]),
        )
        .unwrap();

        assert_eq!(
            injected,
            Value::parse_from_str("Deep::Inner(Left(Mode::Single(9)))", &deep_ty).unwrap()
        );
    }

    #[test]
    fn inject_value_enum_variant_named_left() {
        let deep_ty = nested_enum_type();

        // "Right" selects the Either branch; "Left" selects Wallet's variant.
        let witness = Arc::new(Value::parse_from_str("Deep::Inner(Right(Wallet::Left(5)))", &deep_ty).unwrap());

        let injected =
            WtnsInjector::inject_value(&witness, &deep_ty, &["Inner", "Right", "Left"], Value::u32(9)).unwrap();

        assert_eq!(
            injected,
            Value::parse_from_str("Deep::Inner(Right(Wallet::Left(9)))", &deep_ty).unwrap()
        );
    }

    #[test]
    fn inject_value_enum_payload_of_enum() {
        // Outer::Wrapped(Inner): an Enum stack item wrapping another Enum
        // stack item with no container in between.
        let outer_ty = witness_type(
            "
enum Inner {
    Off,
    On(u32),
}

enum Outer {
    Skip,
    Wrapped(Inner),
}

fn main() {
    let outer: Outer = witness::OUTER;
}
",
        );

        let witness = Arc::new(Value::parse_from_str("Outer::Wrapped(Inner::On(0))", &outer_ty).unwrap());

        let injected = WtnsInjector::inject_value(&witness, &outer_ty, &["Wrapped", "On"], Value::u32(9)).unwrap();

        assert_eq!(
            injected,
            Value::parse_from_str("Outer::Wrapped(Inner::On(9))", &outer_ty).unwrap(),
        );
    }

    #[test]
    fn inject_value_nested_enum_array_payload() {
        let deep_ty = nested_enum_type();
        let (mode_ty, _) = inner_either_types(&deep_ty);

        let witness = Arc::new(Value::parse_from_str("Deep::Bag([Mode::Off, Mode::Off])", &deep_ty).unwrap());

        let injected = WtnsInjector::inject_value(
            &witness,
            &deep_ty,
            &["Bag", "1"],
            enum_variant(&mode_ty, "Single", vec![Value::u32(9)]),
        )
        .unwrap();

        assert_eq!(
            injected,
            Value::parse_from_str("Deep::Bag([Mode::Off, Mode::Single(9)])", &deep_ty).unwrap()
        );
    }
}
