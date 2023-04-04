use std::collections::HashMap;

use log::{debug, info, warn};

use crate::class_manager::ResolvedClass;
use crate::native_methods::NativeMethodsRegistry;
use crate::time::{get_current_time_millis, get_nano_time};
use crate::value::Value::Long;
use crate::{
    call_stack::CallStack,
    class::{ClassId, ClassRef},
    class_and_method::ClassAndMethod,
    class_manager::ClassManager,
    class_path::ClassPathParseError,
    gc::ObjectAllocator,
    value::{ObjectRef, Value},
    vm_error::VmError,
};

#[derive(Debug, Default)]
pub struct Vm<'a> {
    /// Responsible for allocating and storing classes
    class_manager: ClassManager<'a>,

    /// Responsible for allocating objects
    object_allocator: ObjectAllocator<'a>,

    /// To model static fields, we will create one special instance of each class
    /// and we will store it in this map
    statics: HashMap<ClassId, ObjectRef<'a>>,

    /// Stores native methods
    pub native_methods_registry: NativeMethodsRegistry<'a>,

    pub printed: Vec<Value<'a>>, // Temporary, used for testing purposes
}

impl<'a> Vm<'a> {
    pub fn new() -> Self {
        let mut result: Self = Default::default();
        result.register_natives();
        result
    }

    fn register_natives(&mut self) {
        self.native_methods_registry
            .register_temp_print(|vm, _, _, _, args| {
                let arg = args.get(0).ok_or(VmError::ValidationException)?;
                info!(
                    "TEMP implementation of native method: printing value {:?}",
                    args
                );
                vm.printed.push(arg.clone());
                Ok(None)
            });

        self.native_methods_registry.register(
            "java/lang/Object",
            "registerNatives",
            "()V",
            |_, _, _, _, _| Ok(None),
        );
        self.native_methods_registry.register(
            "java/lang/System",
            "registerNatives",
            "()V",
            |_, _, _, _, _| Ok(None),
        );
        self.native_methods_registry.register(
            "java/lang/System",
            "nanoTime",
            "()J",
            |_, _, _, _, _| Ok(Some(Long(get_nano_time()))),
        );
        self.native_methods_registry.register(
            "java/lang/System",
            "currentTimeMillis",
            "()J",
            |_, _, _, _, _| Ok(Some(Long(get_current_time_millis()))),
        );
    }

    pub(crate) fn get_static_instance(&self, class_id: ClassId) -> Option<ObjectRef<'a>> {
        self.statics.get(&class_id).cloned()
    }

    pub fn append_class_path(&mut self, class_path: &str) -> Result<(), ClassPathParseError> {
        self.class_manager.append_class_path(class_path)
    }

    pub fn get_or_resolve_class(
        &mut self,
        stack: &mut CallStack<'a>,
        class_name: &str,
    ) -> Result<ClassRef<'a>, VmError> {
        let class = self.class_manager.get_or_resolve_class(class_name)?;
        if let ResolvedClass::NewClass(classes_to_init) = &class {
            for class_to_init in classes_to_init.to_initialize.iter() {
                let static_instance = self.new_object_of_class(class_to_init);
                self.statics.insert(class_to_init.id, static_instance);
                if let Some(clinit_method) = class_to_init.find_method("<clinit>", "()V") {
                    debug!("invoking {}::<clinit>()", class_to_init.name);

                    // TODO: stack
                    self.invoke(
                        stack,
                        ClassAndMethod {
                            class: class_to_init,
                            method: clinit_method,
                        },
                        None,
                        Vec::new(),
                    )?;
                }
                // TODO: invoke <clinit>
            }
        }
        Ok(class.get_class())
    }

    pub fn find_class_by_id(&self, class_id: ClassId) -> Option<ClassRef<'a>> {
        self.class_manager.find_class_by_id(class_id)
    }

    pub fn resolve_class_method(
        &mut self,
        call_stack: &mut CallStack<'a>,
        class_name: &str,
        method_name: &str,
        method_type_descriptor: &str,
    ) -> Result<ClassAndMethod<'a>, VmError> {
        self.get_or_resolve_class(call_stack, class_name)
            .and_then(|class| {
                class
                    .find_method(method_name, method_type_descriptor)
                    .map(|method| ClassAndMethod { class, method })
                    .ok_or(VmError::ClassNotFoundException(class_name.to_string()))
            })
    }

    // TODO: do we need it?
    pub fn allocate_call_stack(&self) -> CallStack<'a> {
        CallStack::new()
    }

    pub fn invoke(
        &mut self,
        call_stack: &mut CallStack<'a>,
        class_and_method: ClassAndMethod<'a>,
        object: Option<ObjectRef<'a>>,
        args: Vec<Value<'a>>,
    ) -> Result<Option<Value<'a>>, VmError> {
        if class_and_method.method.is_native() {
            let native_callback = self.native_methods_registry.get_method(&class_and_method);
            return if let Some(native_callback) = native_callback {
                native_callback(self, call_stack, class_and_method, object, args)
            } else {
                warn!(
                    "cannot resolve native method {}::{} {}",
                    class_and_method.class.name,
                    class_and_method.method.name,
                    class_and_method.method.type_descriptor
                );
                Err(VmError::NotImplemented)
            };
        }

        let frame = call_stack.add_frame(class_and_method, object, args)?;
        let result = frame.borrow_mut().execute(self, call_stack);
        call_stack.pop_frame()?;
        result
    }

    pub fn new_object(
        &mut self,
        call_stack: &mut CallStack<'a>,
        class_name: &str,
    ) -> Result<ObjectRef<'a>, VmError> {
        let class = self.get_or_resolve_class(call_stack, class_name)?;
        Ok(self.new_object_of_class(class))
    }

    pub fn new_object_of_class(&mut self, class: ClassRef<'a>) -> ObjectRef<'a> {
        debug!("allocating new instance of {}", class.name);
        self.object_allocator.allocate(class)
    }

    pub fn debug_stats(&self) {
        debug!(
            "VM classes={:?}, objects = {:?}",
            self.class_manager, self.object_allocator
        )
    }
}
