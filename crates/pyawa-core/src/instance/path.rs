//! **VM 侧 POSIX 路径解析**（`SPEC-capabilities.md` 的 `CP-21`／`CP-22`／`CP-32`）。
//!
//! 职责（`CP-7`：**权威在 provider、判定在 VM**）：把脚本给的**名字**解析成 provider 能直接执行的
//! **安全名字**。provider 只回答"这个能力对象被允许做什么"，**不再**做 `..`／符号链接的夹根判定
//! （`CP-32`：provider 侧禁止再度解释绝对路径）。
//!
//! # 解析口径
//!
//! 在**guest 绝对路径空间**里解析（`/` ＝ 根能力的根）：
//! 1. **起点**：绝对名字从 `/` 起；相对名字从**实例 `cwd`**（`CP-22`：实例状态）起；
//! 2. **词法归一**：`.` 丢弃、`..` 弹一层、**到 `/` 就停**（chroot：`/..` 仍是 `/`）；
//! 3. **逐段符号链接解析**：每一段先问 provider 的 `readlink` 槽——
//!    - 成功（是符号链接）⇒ 目标为绝对则从 `/` 重来、相对则相对**链接所在目录**继续；
//!      深度上限 [`MAX_SYMLINK_DEPTH`]（Linux 的 40）⇒ 超限报 `ELOOP` 形状；
//!    - `EINVAL`／`ENOENT`／`ENOTDIR` ⇒ **不是**符号链接（`lstat` 语义的"普通段"），照常前进；
//!      其余机器错误原样上抛；
//! 4. **出参**：guest 绝对名字（**含前导 `/`**，**不含越界 `..`**）。provider 拿到的名字永远
//!    落在根能力之内 ⇒ 名字里不再需要"再归一一次"。
//!
//! **叶子语义**：`open`／`stat` 要**跟随**最后一段（`follow_leaf = true`）；
//! `readlink`／`unlink`／`rmdir`／`mkdir`／`rename` 要**动链接本身**（`follow_leaf = false`）。
//!
//! # `cwd`（`CP-22`）
//!
//! `cwd` 是**实例状态**（[`Instance::set_path_cwd`]／[`Instance::path_cwd`]）；初值在首次解析时取
//! **宿主进程 cwd**（`std::env::current_dir`）—— 这是**过渡口径**：`CP-24` 说 cwd 这类残余权威
//! **禁止**绕过能力层，正路是宿主在创建实例时**显式**给出（`CP-20` 根能力一起定义）。
//! 受限 provider 的宿主应显式设 `"/"`（chroot 的根）；不设也能工作（解析在 guest 空间夹根，
//! provider 只做根映射），但"看起来合法"的 `os.getcwd()` 值要等 `CP-22` 的 stdlib 接线。
//!
//! # 与 provider 的名字形态（如实记）
//!
//! 交给 provider 的是 **guest 绝对名字**（如 `/a/b`）。三种 provider 对它的解释：
//! - 真实机器（`PosixFs`，根 `/`）：guest 绝对名 ＝ 宿主绝对名；
//! - 受限（`RestrictedFs`）：根映射（`/a/b` ⇒ `<root>/a/b`；额外只读根按前缀映射）；
//! - 朴素测试（`NaiveFs`）：去掉前导 `/` 后直接 `root.join`，**不做任何越界检查**。
//!
//! 安全性的来源因此只剩本模块（这正是 `T-CP-4` 的强验收形态）：本模块保证名字里没有越界
//! `..`、没有未经解析的逃逸符号链接。
//!
//! # 如实缺口（不假装 ✓）
//!
//! 1. **`readlink` 槽缺失的降级**：provider 没提供 `readlink` 槽（`CP-3`：该操作未实现）时，
//!    本模块**无法识别符号链接** ⇒ 按普通段放行（纯词法解析：`..` 夹根仍生效，但**逃逸链接
//!    探测不到**）。`CP-21` 的"链接夹根"因此**依赖 provider 提供 `readlink` 槽** ——
//!    真实 `PosixFs`／受限 `RestrictedFs` 都提供 ✓；只实现部分槽位的宿主需自己知道这条边界。
//! 2. **"逐段 Dir 能力链"的理想形态未实现**：`DESIGN.md` §7.1 的示例是
//!    `VM 解析 → provider: Dir.open("b")`（沿根能力逐段下降、每段都过能力对象）；v1 用
//!    **guest 绝对名 ＋ provider 根映射**（`RestrictedFs` 把 `/a/b` 映射到 `<root>/a/b`）。
//!    行为上满足 `T-CP-4`（安全由本模块保证），但机制是**简化档**：provider 仍能看到整条
//!    guest 名字，而不是"一段名字 ＋ 一个目录能力"。`CP-21`／`CP-32` 的机制债记在这里，
//!    不因 `T-CP-4` 绿而声称理想形态已达成。
//! 3. **`cwd` 初值是过渡口径**：未显式设置时取宿主进程 cwd（`CP-24` 的残余权威）；正路是宿主
//!    创建实例时显式给（`CP-20` 根能力一起定义）。`os.getcwd`／`os.chdir` 的 stdlib 面本轮
//!    只做核心状态（`CP-22` 部分完成，见 `posix_module` 的既有注释）。

use super::*;

/// 符号链接的解析深度上限（Linux `MAXSYMLINKS = 40`；超限 ⇒ `ELOOP` 形状，`CP-21`）。
pub const MAX_SYMLINK_DEPTH: usize = 40;

/// 平台 errno 表的**兜底值**（只在没有 `pa_create` 注入的裸 `Instance` 单元测试里用到；
/// 正式实例一律按 `CM-20` 从平台表按名字取）。
const FALLBACK_ENOENT: i32 = 2;
const FALLBACK_EINVAL: i32 = 22;
const FALLBACK_ENOTDIR: i32 = 20;
const FALLBACK_ELOOP: i32 = 40;

impl Instance {
    /// 当前**实例 cwd**（guest 绝对形态，`CP-22`；**借用后复制**）。
    ///
    /// 尚未显式设置时惰性取宿主进程 cwd 并记住（过渡口径见模块文档）。
    pub fn path_cwd(&self) -> Vec<u8> {
        if let Some(cwd) = self.path_cwd.borrow().as_ref() {
            return cwd.clone();
        }
        let ambient = std::env::current_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "/".to_owned());
        let absolute = normalize_lexically(ambient.as_bytes());
        *self.path_cwd.borrow_mut() = Some(absolute.clone());
        absolute
    }

    /// 显式设置**实例 cwd**（`CP-22`；宿主在建实例时给出，受限 provider 应设 `"/"`）。
    ///
    /// 入参按 guest 空间解释：相对值从 `/` 起（不做平台 cwd 拼接），随后词法归一。
    pub fn set_path_cwd(&self, cwd: &[u8]) {
        *self.path_cwd.borrow_mut() = Some(normalize_lexically(cwd));
    }

    /// **解析一个名字**（跟随最后一段；`open`／`stat` 用）⇒ 安全的 guest 绝对名字。
    pub fn resolve_path(&self, name: &[u8]) -> Result<Vec<u8>, CapabilityCallError> {
        self.resolve_guest_path(name, true)
    }

    /// **解析一个名字**（**不**跟随最后一段；`readlink`／`unlink`／`rmdir`／`mkdir`／`rename` 用）。
    pub fn resolve_path_parent(&self, name: &[u8]) -> Result<Vec<u8>, CapabilityCallError> {
        self.resolve_guest_path(name, false)
    }

    /// 解析主循环（口径见模块文档）。
    fn resolve_guest_path(
        &self,
        name: &[u8],
        follow_leaf: bool,
    ) -> Result<Vec<u8>, CapabilityCallError> {
        // 空名字 ⇒ 参照 `open("")` 是 `ENOENT`（不是 cwd）。
        if name.is_empty() {
            return Err(CapabilityCallError::Machine(self.path_errno(
                "ENOENT",
                FALLBACK_ENOENT,
            )));
        }
        // 起点：绝对名字从 `/` 起；相对名字从实例 cwd 起。
        let mut queue: std::collections::VecDeque<Vec<u8>> = std::collections::VecDeque::new();
        if name.starts_with(b"/") {
            push_components(&mut queue, name);
        } else {
            push_components(&mut queue, &self.path_cwd());
            push_components(&mut queue, name);
        }
        // `resolved` 是"已经走过的 guest 绝对路径"的分量栈（永远不含 `.`／`..`）。
        let mut resolved: Vec<Vec<u8>> = Vec::new();
        let mut followed = 0usize;
        while let Some(component) = queue.pop_front() {
            match component.as_slice() {
                b"" | b"." => continue,
                b".." => {
                    // chroot：`/..` 仍是 `/`（夹住，不越界）。
                    let _ = resolved.pop();
                    continue;
                }
                _ => {}
            }
            let is_last = queue.is_empty();
            if is_last && !follow_leaf {
                // 不动最后一段（`readlink`／`unlink` 一族）：父目录已解析完，最后一段原样保留。
                resolved.push(component);
                continue;
            }
            let candidate = join_guest(&resolved, Some(&component));
            match self.fs_readlink_raw(&candidate) {
                Ok(target) => {
                    followed += 1;
                    if followed > MAX_SYMLINK_DEPTH {
                        return Err(CapabilityCallError::Machine(self.path_errno(
                            "ELOOP",
                            FALLBACK_ELOOP,
                        )));
                    }
                    if target.is_empty() {
                        return Err(CapabilityCallError::Machine(self.path_errno(
                            "ENOENT",
                            FALLBACK_ENOENT,
                        )));
                    }
                    // 目标的解释：绝对 ⇒ 从 `/` 重来；相对 ⇒ 相对链接所在目录（= 当前 resolved）。
                    if target.starts_with(b"/") {
                        resolved.clear();
                    }
                    let mut target_components: std::collections::VecDeque<Vec<u8>> =
                        std::collections::VecDeque::new();
                    push_components(&mut target_components, &target);
                    // 目标分量插到队首（深度优先，保持 POSIX 的从左到右序）。
                    while let Some(part) = target_components.pop_back() {
                        queue.push_front(part);
                    }
                }
                Err(CapabilityCallError::Machine(errno))
                    if errno == self.path_errno("EINVAL", FALLBACK_EINVAL)
                        || errno == self.path_errno("ENOENT", FALLBACK_ENOENT)
                        || errno == self.path_errno("ENOTDIR", FALLBACK_ENOTDIR) =>
                {
                    // 不是符号链接（或尚未存在）⇒ 普通段（`CP-21` 的"EINVAL 视为普通段"）。
                    resolved.push(component);
                }
                // **`CP-3`**：provider 没实现 `readlink` 槽时，只有"符号链接探测"退化为
                // **纯词法**解析（`..` 夹根仍然生效；**逃逸符号链接探测不到** ⇒ 由 provider 自负，
                // 如实记：要满足 `CP-21` 的链接面，provider 必须提供 `readlink` 槽）。
                Err(
                    CapabilityCallError::NotImplemented | CapabilityCallError::NotRegistered,
                ) => resolved.push(component),
                Err(error) => return Err(error),
            }
        }
        Ok(join_guest(&resolved, None))
    }

    /// 按**名字**取平台 errno（`CM-20`）；没有注入表的裸实例用兜底值。
    fn path_errno(&self, name: &str, fallback: i32) -> i32 {
        self.platform_constant(name)
            .map(|value| value as i32)
            .unwrap_or(fallback)
    }
}

/// 把路径切成**分量**（忽略 `/`；`.` 与 `..` 保留给解析循环处理）。
fn push_components(queue: &mut std::collections::VecDeque<Vec<u8>>, path: &[u8]) {
    for component in path.split(|byte| *byte == b'/') {
        queue.push_back(component.to_vec());
    }
}

/// 由"已解析分量栈"拼 guest 绝对路径；`None` ⇒ 路径自身（根是 `"/"`）。
fn join_guest(resolved: &[Vec<u8>], leaf: Option<&[u8]>) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    out.push(b'/');
    for (index, component) in resolved.iter().enumerate() {
        if index > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(component);
    }
    if let Some(leaf) = leaf {
        if !resolved.is_empty() {
            out.push(b'/');
        }
        out.extend_from_slice(leaf);
    }
    out
}

/// **纯词法**归一（不碰 provider；给 cwd 初值/设置用）：`.` 丢弃、`..` 弹栈、到 `/` 就停。
fn normalize_lexically(path: &[u8]) -> Vec<u8> {
    let mut stack: Vec<Vec<u8>> = Vec::new();
    for component in path.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                let _ = stack.pop();
            }
            other => stack.push(other.to_vec()),
        }
    }
    join_guest(&stack, None)
}
