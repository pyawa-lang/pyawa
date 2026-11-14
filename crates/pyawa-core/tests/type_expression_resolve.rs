//! **TS 注解求值层**（`SPEC-type-system.md` §16 的 `TS-52`／`TS-53`；M3，2026-11-14）。
//!
//! 钉住：
//! 1. 名称解析次序（globals → builtins → 类型注册表）与点号属性链；
//! 2. `typing` 构造（`Optional`/`Union`/`Callable`/`Annotated`/`List`/`Dict`/`Set`/`Tuple`）
//!    到 [`SemanticType`] 的映射；
//! 3. `BC-24` 静态标签投影（`to_static_label`）与 `AB-69` 同口径；
//! 4. 求值失败如实分类（`Unknown`／`Deferred`／`Invalid`），语法错与求值错分开。

mod common;

use pyawa_core::type_expr::{
    evaluate_type_expression, resolve_type_expr, ResolveError, ResolveScope, SemanticType,
    TypeAnnotationError,
};
use pyawa_core::TypeExpr;
use common::Vm;

fn scope<'a>(vm: &'a Vm) -> ResolveScope<'a> {
    ResolveScope::new(&vm.instance)
}

fn resolve(vm: &Vm, source: &str) -> SemanticType {
    evaluate_type_expression(&scope(vm), source)
        .unwrap_or_else(|error| panic!("{source:?} 应当求值成功，实际 {error}"))
}

fn named(vm: &Vm, name: &str) -> SemanticType {
    SemanticType::Named(vm.instance.type_named(name).unwrap_or_else(|| panic!("{name} 应已登记")))
}

#[test]
fn resolves_names_and_containers() {
    let vm = Vm::new();
    assert_eq!(resolve(&vm, "int"), named(&vm, "int"));
    assert_eq!(resolve(&vm, "None"), SemanticType::NoneType);
    assert_eq!(resolve(&vm, "Any"), SemanticType::Any);
    assert_eq!(resolve(&vm, "list"), named(&vm, "list"));
    assert_eq!(
        resolve(&vm, "list[int]"),
        SemanticType::ListOf(Box::new(named(&vm, "int")))
    );
    assert_eq!(
        resolve(&vm, "dict[str, int]"),
        SemanticType::DictOf(Box::new(named(&vm, "str")), Box::new(named(&vm, "int")))
    );
    // `typing` 大写名与内置小写名同一映射。
    assert_eq!(resolve(&vm, "List[int]"), resolve(&vm, "list[int]"));
    assert_eq!(resolve(&vm, "Dict[str, int]"), resolve(&vm, "dict[str, int]"));
    // tuple/set 走泛型擦除（`Apply(base, …)`）——边界只核基底。
    assert_eq!(
        resolve(&vm, "tuple[int, ...]"),
        SemanticType::Apply(Box::new(named(&vm, "tuple")), vec![named(&vm, "int")])
    );
    assert_eq!(
        resolve(&vm, "set[int]"),
        SemanticType::Apply(Box::new(named(&vm, "set")), vec![named(&vm, "int")])
    );
}

#[test]
fn resolves_unions_and_optionals() {
    let vm = Vm::new();
    let int = named(&vm, "int");
    let str_ty = named(&vm, "str");
    assert_eq!(
        resolve(&vm, "int | None"),
        SemanticType::Optional(Box::new(int.clone()))
    );
    assert_eq!(resolve(&vm, "int?"), resolve(&vm, "int | None"));
    assert_eq!(
        resolve(&vm, "int | str"),
        SemanticType::Union(vec![int.clone(), str_ty.clone()])
    );
    assert_eq!(
        resolve(&vm, "Optional[int]"),
        SemanticType::Optional(Box::new(int.clone()))
    );
    assert_eq!(
        resolve(&vm, "Union[int, str]"),
        SemanticType::Union(vec![int.clone(), str_ty.clone()])
    );
    // `Union[T]` ≡ `T`；重复成员去重。
    assert_eq!(resolve(&vm, "Union[int]"), int);
    assert_eq!(resolve(&vm, "int | int"), int);
    // `typing.Optional` 点号名也识别。
    assert_eq!(resolve(&vm, "typing.Optional[int]"), resolve(&vm, "Optional[int]"));
}

#[test]
fn resolves_callable_and_annotated() {
    let vm = Vm::new();
    let int = named(&vm, "int");
    let str_ty = named(&vm, "str");
    assert_eq!(
        resolve(&vm, "Callable[[int, str], None]"),
        SemanticType::Callable {
            params: vec![int.clone(), str_ty],
            ret: Box::new(SemanticType::NoneType),
        }
    );
    // `Callable[..., R]`：形参可变（v1 只核可调用，与 `AB-69` 一致）。
    assert_eq!(
        resolve(&vm, "Callable[..., int]"),
        SemanticType::Callable {
            params: Vec::new(),
            ret: Box::new(int.clone()),
        }
    );
    // `Annotated[T, …]`：只看第一个实参，元数据不参与类型判定。
    assert_eq!(resolve(&vm, "Annotated[int, \"meta\"]"), int);
}

#[test]
fn resolves_type_variables_and_dotted_paths() {
    let vm = Vm::new();
    let parameters = vec!["T".to_owned()];
    let scope = ResolveScope {
        instance: &vm.instance,
        globals: None,
        tparams: &parameters,
    };
    assert_eq!(
        evaluate_type_expression(&scope, "T").expect("T 是声明的类型变量"),
        SemanticType::Var("T".to_owned())
    );
    assert_eq!(
        evaluate_type_expression(&scope, "list[T]").expect("list[T]"),
        SemanticType::ListOf(Box::new(SemanticType::Var("T".to_owned())))
    );
    // 点号属性链：`type.__class__` 是 `type`（类型对象；`__base__` 单数本层未实现）。
    assert_eq!(
        evaluate_type_expression(&scope, "type.__class__").expect("type.__class__"),
        named(&vm, "type")
    );
}

#[test]
fn resolves_forward_refs_and_globals_aliases() {
    let vm = Vm::new();
    let globals = vm.instance.new_dict();
    // globals 里的别名优先于类型注册表（`TS-52` 的次序）。
    let alias = vm.instance.type_value(vm.instance.type_named("int").expect("int"));
    vm.instance.dict_set(globals, "Alias", alias);
    let scope = ResolveScope {
        instance: &vm.instance,
        globals: Some(globals),
        tparams: &[],
    };
    assert_eq!(
        evaluate_type_expression(&scope, "Alias").expect("Alias 在 globals 里"),
        named(&vm, "int")
    );
    // 字符串前向引用：能查到 ⇒ 类型；查不到 ⇒ `Deferred`（可延迟重试，不猜 `Any`）。
    assert_eq!(
        evaluate_type_expression(&scope, "\"Alias\"").expect("前向引用可解析"),
        named(&vm, "int")
    );
    assert_eq!(
        evaluate_type_expression(&scope, "\"Later\""),
        Err(TypeAnnotationError::Resolve(ResolveError::Deferred(
            "Later".to_owned()
        )))
    );
}

#[test]
fn static_labels_match_the_ab69_projection() {
    let vm = Vm::new();
    let label = |source: &str| resolve(&vm, source).to_static_label();
    assert_eq!(label("int").as_deref(), Some("int"));
    assert_eq!(label("list[int]").as_deref(), Some("list"));
    assert_eq!(label("dict[str, int]").as_deref(), Some("dict"));
    assert_eq!(label("Any").as_deref(), Some("Any"));
    assert_eq!(label("tuple[int, ...]").as_deref(), Some("tuple"));
    assert_eq!(label("int | None"), None);
    assert_eq!(label("Callable[[int], int]"), None);
}

#[test]
fn evaluation_errors_are_classified() {
    let vm = Vm::new();
    // 语法错 ⇒ `Syntax`（不是求值错）。
    assert!(matches!(
        evaluate_type_expression(&scope(&vm), "int + 1"),
        Err(TypeAnnotationError::Syntax(_))
    ));
    // 未知名字 ⇒ `Unknown`。
    assert_eq!(
        evaluate_type_expression(&scope(&vm), "NoSuchType"),
        Err(TypeAnnotationError::Resolve(ResolveError::Unknown(
            "NoSuchType".to_owned()
        )))
    );
    // 裸 `Optional`/`Callable` 没有类型含义 ⇒ `Invalid`。
    for source in ["Optional", "Callable", "Union", "Annotated"] {
        assert!(
            matches!(
                evaluate_type_expression(&scope(&vm), source),
                Err(TypeAnnotationError::Resolve(ResolveError::Invalid(_)))
            ),
            "{source} 应当 Invalid"
        );
    }
    // 实参个数不对 ⇒ `Invalid`。
    for source in ["list[int, int]", "dict[int]", "Callable[int]", "Optional[int, int]"] {
        assert!(
            matches!(
                evaluate_type_expression(&scope(&vm), source),
                Err(TypeAnnotationError::Resolve(ResolveError::Invalid(_)))
            ),
            "{source} 应当 Invalid"
        );
    }
    // `resolve_type_expr` 直接吃语法树（不经字符串）。
    assert_eq!(
        resolve_type_expr(&scope(&vm), &TypeExpr::Name("missing".to_owned())),
        Err(ResolveError::Unknown("missing".to_owned()))
    );
}
