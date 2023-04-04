use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    rc::Rc,
};

use rjvm_reader::field_type::{BaseType, FieldType};

use crate::class::{Class, ClassId, ClassRef};

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Value<'a> {
    #[default]
    Uninitialized,
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Object(ObjectRef<'a>),
    Null, // TODO: should this be merged with Object and use an Option?

    // TODO: avoid RC and use garbage collector to allocate
    Array(FieldType, ArrayRef<'a>),
    // TODO: return address
}

#[derive(Clone, PartialEq)]
pub struct ObjectValue<'a> {
    pub class_id: ClassId,
    fields: RefCell<Vec<Value<'a>>>,
}

impl<'a> ObjectValue<'a> {
    pub fn new(class: &Class<'a>) -> Self {
        let fields = (0..class.num_total_fields)
            .map(|index| {
                let field = class.field_at_index(index).unwrap();
                match &field.type_descriptor {
                    FieldType::Base(base_type) => match base_type {
                        BaseType::Byte => Value::Int(0),
                        BaseType::Char => Value::Int(0),
                        BaseType::Double => Value::Double(0f64),
                        BaseType::Float => Value::Float(0f32),
                        BaseType::Int => Value::Int(0),
                        BaseType::Long => Value::Long(0),
                        BaseType::Short => Value::Int(0),
                        BaseType::Boolean => Value::Int(0),
                    },
                    FieldType::Object(_) => Value::Null,
                    FieldType::Array(_) => Value::Null,
                }
            })
            .collect();
        Self {
            class_id: class.id,
            fields: RefCell::new(fields),
        }
    }

    pub fn set_field(&self, index: usize, value: Value<'a>) {
        self.fields.borrow_mut()[index] = value;
    }

    pub fn get_field(&self, index: usize) -> Value<'a> {
        self.fields.borrow()[index].clone()
    }
}

pub type ObjectRef<'a> = &'a ObjectValue<'a>;
pub type ArrayRef<'a> = Rc<RefCell<Vec<Value<'a>>>>;

impl<'a> Value<'a> {
    pub fn matches_type<'b, T>(&self, expected_type: FieldType, class_resolver: T) -> bool
    where
        T: FnOnce(ClassId) -> Option<ClassRef<'b>>,
    {
        match self {
            Value::Uninitialized => false,
            Value::Int(_) => match expected_type {
                FieldType::Base(base_type) => base_type == BaseType::Int,
                _ => false,
            },
            Value::Long(_) => match expected_type {
                FieldType::Base(base_type) => base_type == BaseType::Long,
                _ => false,
            },
            Value::Float(_) => match expected_type {
                FieldType::Base(base_type) => base_type == BaseType::Float,
                _ => false,
            },
            Value::Double(_) => match expected_type {
                FieldType::Base(base_type) => base_type == BaseType::Double,
                _ => false,
            },

            Value::Object(object_ref) => match expected_type {
                // TODO: with multiple class loaders, we should check the class identity,
                //  not the name, since the same class could be loaded by multiple class loader

                // TODO: we should check super classes
                FieldType::Object(class_name) => {
                    let value_class = class_resolver(object_ref.class_id);
                    if let Some(class_ref) = value_class {
                        class_ref.name == class_name
                    } else {
                        false
                    }
                }
                _ => false,
            },

            Value::Null => false,

            Value::Array(field_type, _) => match expected_type {
                FieldType::Array(expected_field_type) => *field_type == *expected_field_type,
                _ => false,
            },
        }
    }
}

impl<'a> Debug for ObjectValue<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "class: {} fields {:?}", self.class_id, self.fields)
    }
}
