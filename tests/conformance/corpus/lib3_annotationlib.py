# `MS-13` ② 的 `Lib/` 语料（第三批·批量覆盖）：被测代码即逐字同步的
# `Lib/annotationlib.py`（import 驱动）。
# 探针只读模块面：`__name__` ＋ 参照公开名在本侧命名空间中的存在性（有 `__all__` 时一并比对）。
import annotationlib

name = annotationlib.__name__
surface = str(sorted(n for n in ['Format', 'ForwardRef', 'annotations_to_string', 'ast', 'builtins', 'call_annotate_function', 'call_evaluate_function', 'enum', 'get_annotate_from_class_namespace', 'get_annotations', 'keyword', 'sys'] if hasattr(annotationlib, n)))
declared = str(sorted(annotationlib.__all__))
