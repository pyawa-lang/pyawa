//! `emitter` 的 `stmt_imports` 域：`import`／`from … import` 的发射。

use super::*;

impl Emitter {
    pub(crate) fn emit_statement_imports(&mut self, statement: &Statement, _rest: &[Statement]) -> Result<(), CompileError> {
        match statement {
                Statement::Import { items, span } => {
                    for (module, alias) in items {
                        self.intern_literal(Constant::Int(0));
                        self.emit_at(
                            *span,
                            opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                            0,
                        );
                        let none_index = self.intern_constant(Constant::None);
                        self.emit_indexed(*span, "LOAD_CONST", none_index);
                        let module_index = self.intern_name(module);
                        self.emit_indexed(*span, "IMPORT_NAME", module_index);
                        match alias {
                            // **含点的模块**要逐段 `IMPORT_FROM`**（round 98 真 bug 修 ✗）：
                            // 参照对 `import a.b.c as x` 发的是
                            // `IMPORT_NAME a.b.c; IMPORT_FROM b; SWAP 2; POP_TOP; IMPORT_FROM c;
                            //  STORE x; POP_TOP` ✓ —— **中间段也要取一次**（每取一段用 SWAP/POP 丢掉
                            // 上一层模块 ✓）。先前只取**最后一段** ✗ ⇒ 对三段以上名字，`IMPORT_FROM` 作用在
                            // **顶层模块**上 ⇒ `ImportError: cannot import name 'ElementTree' from 'xml'` ✗
                            //（官方 `test_xml_etree`／`test_xml_etree_c` 的 `import_fresh_module` 一上来就撞 ✓；
                            //  二段名字（`import a.b as x`）参照只有一次 `IMPORT_FROM` ✓ 与本层原逻辑一致 ✓）。
                            Some(alias) if module.contains('.') => {
                                let segments: Vec<&str> = module.split('.').collect();
                                for segment in &segments[1..segments.len().saturating_sub(1)] {
                                    let segment_index = self.intern_name(segment);
                                    self.emit_indexed(*span, "IMPORT_FROM", segment_index);
                                    self.emit_at(
                                        *span,
                                        opcode::opcode("SWAP").expect("SWAP 在表里"),
                                        2,
                                    );
                                    self.emit_at(
                                        *span,
                                        opcode::opcode("POP_TOP").expect("POP_TOP 在表里"),
                                        0,
                                    );
                                }
                                let last = segments.last().copied().unwrap_or(module);
                                let last_index = self.intern_name(last);
                                self.emit_indexed(*span, "IMPORT_FROM", last_index);
                                self.store_target(*span, alias);
                                self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                            }
                            Some(alias) => self.store_target(*span, alias),
                            None => {
                                let top = module.split('.').next().unwrap_or(module);
                                self.store_target(*span, top);
                            }
                        }
                    }
                    // 收尾跟着**本条语句**的跨度（实测 `def f():\n    import a\n` 的收尾是 `(2,2)`）
                    self.epilogue_span = *span;
                    Ok(())
                }
                // **`from … import …`**（逐条实测）：`LOAD_SMALL_INT <层级>; LOAD_CONST (<名字>, …);
                //   IMPORT_NAME <模块>` ＋ 逐名字 `IMPORT_FROM; STORE` ＋ 末尾一条 `POP_TOP`；
                //   `*` 走 `CALL_INTRINSIC_1 2`（`INTRINSIC_IMPORT_STAR`）再 `POP_TOP`。
                Statement::ImportFrom {
                    module,
                    level,
                    names,
                    star,
                    span,
                } => {
                    self.intern_literal(Constant::Int(i64::from(*level)));
                    self.emit_at(
                        *span,
                        opcode::opcode("LOAD_SMALL_INT").expect("LOAD_SMALL_INT 在表里"),
                        *level,
                    );
                    let fromlist: Vec<String> = if *star {
                        vec!["*".to_owned()]
                    } else {
                        names.iter().map(|(name, _)| name.clone()).collect()
                    };
                    let list_index = self.intern_constant(Constant::Names(fromlist));
                    self.emit_indexed(*span, "LOAD_CONST", list_index);
                    let module_index = self.intern_name(module);
                    self.emit_indexed(*span, "IMPORT_NAME", module_index);
                    if *star {
                        self.emit_at(
                            *span,
                            opcode::opcode("CALL_INTRINSIC_1").expect("CALL_INTRINSIC_1 在表里"),
                            2,
                        );
                        self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                    } else {
                        for (name, alias) in names {
                            let name_index = self.intern_name(name);
                            self.emit_indexed(*span, "IMPORT_FROM", name_index);
                            self.store_target(*span, alias.as_deref().unwrap_or(name));
                        }
                        self.emit_at(*span, opcode::opcode("POP_TOP").expect("POP_TOP 在表里"), 0);
                    }
                    self.epilogue_span = *span;
                    Ok(())
                }
            _ => unreachable!("emit_statement_imports 只接收 stmt_imports 族的 Statement"),
        }
    }
}
