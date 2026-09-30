# SPEC — 能力层接口

> 规范性文件 · 状态：M1 前置规格 v0
> 读者：实现能力接口、宿主 provider 与 VM 侧适配层的开发者
> **冲突时的优先级**：`REQUIREMENTS.md`（决策）> 本文件（执行细则）> `DESIGN.md`（架构论证）
> 依赖：`SPEC-object-model.md`（实例、引用计数、宿主对象）

规范用语：**必须** / **禁止** / **建议** / **可选**。硬约束编号 **`CP-n`**，验收测试可直接引用编号。

---

## 1. 范围与依赖

本文件规定**能力层的接口形状**：域如何切分、每域有哪些槽位、句柄生命周期、哪些调用可异步化与如何取消、以及"未实现"如何表达。

**不**规定（明确划界，避免重复与漂移）：

| 内容 | 去处 |
|---|---|
| 为什么"权威在 provider、判定在 VM"（论证） | `DESIGN.md` §7 |
| 能力接口 vtable **如何注册**、错误码编码、跨 ABI 句柄的借用／转移、版本字段与冻结时机 | `SPEC-c-abi.md` |
| 对象头、引用计数协议、宿主对象的表示与 `traverse` | `SPEC-object-model.md` |
| 各 C 模块从 Python 看到的 API 与语义（含机器错误映射到哪个异常类） | `SPEC-c-modules.md` |
| 指令集、帧、编译管线 | `SPEC-bytecode.md` |
| 模式开关、import 钩子、`.pyac` | `SPEC-imports-and-modes.md` |
| 渐进类型的相容关系与边界检查 | `SPEC-type-system.md` |

依赖方向：本文件依赖 `DESIGN.md` §7 与 `REQUIREMENTS.md` 的能力相关决策；`SPEC-c-abi.md`、`SPEC-c-modules.md`、`SPEC-object-model.md` 只引用本文件的 `CP-` 编号。

**槽位归属**：能力函数的集合与编号**唯一归本文件**（`SPEC-INDEX.md` §4）。别处需要新槽位时，**必须**先在本文件领一个 `CP-n` 并写明语义，**禁止**在别处自行定义能力函数。

---

## 2. 上游硬契约（**这一节不是设计，是逐字同步 `Lib/` 推导出的义务**）

`Lib/` 逐字同步、**禁止本地修改**（`DESIGN.md` §9、`REQUIREMENTS.md` 标准库行）⇒ 能力层不是"随便定一层内部接口"，它必须能承载上游纯 Python 模块对 C 层的要求。对本文件的义务：

| 对本层的义务 | 来源 |
|---|---|
| 能力句柄**禁止**泄漏到 Python 层：Python 侧只允许整数 `fd`，且 `fd` 语义下的既有写法必须全部成立 | `DESIGN.md` §2 |
| 错误**必须**是机器错误形状（可还原 `errno`），且与"未实现"分属两类（CP-5／CP-6） | `DESIGN.md` §2、§7 原则 4 |
| provider **必须**能填满 `stat` 一类结构所需的字段 | `DESIGN.md` §7.3、§9 |
| 路径语义**必须**能还原 POSIX；解析责任在本层（CP-21…CP-23） | `DESIGN.md` §7.4 |
| 九域**必须**齐全；缺域只在**调用时**报"未实现"，**禁止**表现为 import 失败 | `DESIGN.md` §7、§9 |

---

## 3. 术语

- **域（domain）**：一组同类外部权威；九域见 `DESIGN.md` §7.3。
- **provider**：宿主（或独立运行时）对某个域的 vtable 实现。
- **能力对象**：provider 给出的**不透明句柄**；凭据即能力，没有全局命名空间（`DESIGN.md` §7.1）。
- **根能力**：受限 provider 交给实例的那个目录／资源能力；绝对路径相对它解析。
- **实例能力表**：每实例一张 `fd → 能力对象` 表；`fd` 就是它的下标（`DESIGN.md` §7.1）。
- **槽位**：vtable 里的一个函数位；为 null 即该操作未实现（CP-3）。
- **取消令牌**：请求取消一个在途调用的凭据（CP-26）。

---

## 4. 域的切分

| 域 | vtable（建议名） | 对应 C 层模块（`DESIGN.md` §7.3） | 异步分类（建议值，仍须按 CP-25 显式声明） |
|---|---|---|---|
| `fs` | `CpFsVtable` | `posix`、`_io` | 可异步化 |
| `net` | `CpNetVtable` | `_socket`、`select` | 可异步化 |
| `proc` | `CpProcVtable` | `posix`（fork/exec）、`_signal` | 可异步化 |
| `clock` | `CpClockVtable` | `time` | 可异步化 |
| `random` | `CpRandomVtable` | `_random`、`os.urandom` | 可异步化 |
| `env` | `CpEnvVtable` | `posix`（`getenv`/`getpid`/`getuid`） | 可异步化 |
| `tty` | `CpTtyVtable` | `termios`、`readline`、`_curses` | 可异步化 |
| `locale` | `CpLocaleVtable` | `_locale`、`_codecs*`、`unicodedata` | 可异步化 |
| `ipc` | `CpIpcVtable` | `_thread`、`mmap`、`_posixshmem` | **按槽位**（`_thread` 触碰 VM 对象 ⇒ 不可异步化） |

- **CP-1** 域集合**必须**与 `DESIGN.md` §7.3 一一对应。新增域**必须**同时升接口版本（CP-30）。
- **CP-2** 某域 vtable 指针为 null ⇒ 该域**整域未实现**：调用报"未实现"（CP-5），**禁止**在创建实例时拒绝。
- **CP-3** 域内单个槽位为 null ⇒ 只有该操作未实现，同域其余槽位不受影响；**禁止**用"创建期扫描并拒绝"替代调用时判定。

---

## 5. 通用调用契约

- **CP-4** 所有外部世界访问**必须**经能力调用。VM 核心与接口 crate **禁止**依赖 `std::fs`／`std::net`／libc，**禁止**出现 `#[cfg(target_os)]`（检查项见 `tests/ci/README.md` 第 3 项）。
- **CP-5** "未实现"的表达：每次调用**必须**返回三种结果之一——**成功**、**机器错误**、**未实现**。**禁止**把"未实现"编码成某个 `errno`，也**禁止**与机器错误共用一条通道（那正是 `DESIGN.md` §2 要求分清的两件事）。
- **CP-6** **无权限语义**：宿主若要拒绝，**必须**返回真实机器错误（`PermissionError`／`FileNotFoundError` 一类），**禁止**发明"权限不足"这一类别（`DESIGN.md` §7 原则 4）。
- **CP-7** **权威在 provider、判定在 VM**：路径规范化、句柄有效性校验**必须**在 VM 侧完成；provider 只回答"这个能力对象被允许做什么"（`DESIGN.md` §7 原则 1）。
- **CP-8** 预算与内存记账**禁止**进入能力接口（`OM-3`）：接口**禁止**出现预算参数、额度查询或"是否超限"的返回。
- **CP-9** 调用**必须**是同步的；"可中断"由 CP-26 的取消令牌实现，**禁止**把接口改成异步回调式（`REQUIREMENTS.md` 能力调用模型行）。
- **CP-10** 接口面只允许**裸数据**与**不透明句柄**：**禁止**泛型／单态化（`REQUIREMENTS.md` 后果 4）；可异步化路径上**禁止**传递或触碰 VM 对象（`DESIGN.md` §3 不变量 4）。
- **CP-11** 每个能力调用点**必须** `catch_unwind`：Rust panic **禁止**穿过 provider 边界，宿主侧异常也**禁止**穿进 VM（`DESIGN.md` §3 不变量 3 的对偶）。
- **CP-12** 接口 crate（能力形状）**必须**只含类型与函数指针，**禁止**含任何真实机器实现；真实机器实现只允许在独立运行时 crate（`DESIGN.md` §7）。
- **CP-13** 每实例**必须**有独立的能力表与根能力；**禁止**进程级能力表、命名空间或单例（`DESIGN.md` §3 不变量 2，同 `OM-1`／`OM-4`）。
- **CP-14** 句柄**禁止**出现在 Python 可见的值里：Python 层只见 `fd`（`int`）及由其派生的整数（第 2 节）。
- **CP-15** 宿主对象／方法的暴露**必须**走本文件这同一套授权模型，**禁止**另立"注册即授予"的旁路（`OM-37`、`DESIGN.md` §8.3）。

---

## 6. 句柄生命周期

> 跨 ABI 的借用／转移语义、有效期，以及"注册"这个动作本身的形状归 `SPEC-c-abi.md`；本节只定能力对象的**语义**生命周期。

- **CP-16** 能力对象**必须**由 provider 创建；其生命周期**至少**覆盖"实例持有它的整段时间"。VM 仍持有时**禁止**回收，也**禁止**要求调用方在同一调用窗口内反复重新获取。
- **CP-17** 外部资源释放的**唯一**入口是能力槽位 `close`：它**必须**存在、**必须**幂等；由谁调用（`os.close`、`_io` 的终结器、实例销毁）归 VM 侧与 `SPEC-c-modules.md`，本文件只要求实例销毁**必须**兜底（CP-19）。
- **CP-18** 对已关闭句柄的再次操作**必须**返回机器错误（`EBADF` 形状），**禁止**是未定义行为。
- **CP-19** 实例销毁**必须**释放全部能力表条目，**不依赖**回收器先跑完（同 `OM-2`）；实例中断**必须**使该实例所有在途的可异步化调用进入取消流程（CP-26），并在中断返回前把能力表恢复到一致状态。
- **CP-20** 根能力**必须**在创建实例时显式给出；**禁止**"未给根能力时默认全权"的兜底——独立运行时的"真实机器"也只是显式注入的一个 provider（`DESIGN.md` §7）。

---

## 7. 路径与根能力

- **CP-21** 路径解析**必须**在 VM 侧完成：绝对路径相对**根能力**解析（chroot 语义），`..` 与符号链接**禁止**跳出根（`DESIGN.md` §7.4）。provider 侧的"路径"只是相对当前能力对象的一层名字。
- **CP-22** `cwd` **必须**是实例状态，且与根能力一起定义；受限 provider 下 `os.getcwd()` **必须**返回一个看起来合法的值（`DESIGN.md` §7.4）。
- **CP-23** provider 未提供 `/proc`／`/dev`／`/sys` 时，访问**必须**表现为 `FileNotFoundError`，**禁止**表现为"未实现"（`DESIGN.md` §2 的两种失败之分）。
- **CP-24** 环境与身份的残余权威（cwd、`getpid`、`getuid`、`time`）**禁止**绕过本层：受限 provider **只能**给受控值或常量（`DESIGN.md` §7.1）。

---

## 8. 可异步化与取消

- **CP-25** 每个槽位**必须**显式声明异步分类（可异步化／不可异步化）。**缺失即注册失败**，**禁止**默认值——默认值就是数据竞争（`DESIGN.md` §7.2）。分类判据本身见 §13-4。
- **CP-26** 可异步化槽位**必须**只收发裸数据与 OS 句柄；其签名**禁止**含 VM 对象类型（编译期可断言，见 `T-CP-9`）。
- **CP-27** 可异步化调用**必须**支持取消令牌：调用方**必须**能请求取消，并**必须**用**可取消的等待**（带超时／轮询）取回结果，**禁止**无限阻塞（`DESIGN.md` §7.2）。
- **CP-28** 取消结果**必须**是三者之一——未开始／已取消／已完成；返回"已取消"之后**禁止**再有该调用的后续副作用。
- **CP-29** 不可异步化槽位**必须**留在 VM 线程（同步窗口）；**禁止**为了"更可中断"把它们送进 worker（`DESIGN.md` §7.2）。
- **CP-30** 域 vtable **必须**带版本／大小字段的位置；字段的具体形状、编码与冻结时机归 `SPEC-c-abi.md`（**临时假设**：注册时校验，版本不符即注册失败）。版本策略未定，见 §13-2。

---

## 9. 每域契约

下面列的槽位是**形状**（名字与语义）；参数怎么摆、错误怎么带、句柄怎么借出**归 `SPEC-c-abi.md`**。逐模块还需要的新槽位，按 §1 的归属规则**先在本文件领号**。

每节末尾的异步分类是**建议值**；实现仍**必须**按 CP-25 显式声明，缺失即注册失败。

### 9.1 `fs` —— 文件系统（`posix`、`_io`）

语义：POSIX 文件／目录语义。权威是**目录能力**：`open` 的相对名字由 VM 解析器沿根能力逐段解析（CP-21）。

| 槽位 | 语义 |
|---|---|
| `open(name, flags, mode)` | 返回文件能力 |
| `stat(name)` / `fstat(file)` | 元信息（字段集见 §2） |
| `listdir(dir)` | 名字序列 |
| `mkdir(name, mode)` / `rmdir(name)` | 目录创建／删除 |
| `unlink(name)` / `rename(a, b)` | 删除／改名 |
| `readlink(name)` / `symlink(target, name)` | 符号链接（解析仍受 CP-21 约束） |
| `chmod(name, mode)` / `utime(name, atime, mtime)` | 元信息修改 |
| `read(file, n)` / `write(file, buf)` | 文件读写 |
| `seek(file, offset, whence)` / `tell(file)` | 位置 |
| `truncate(file, size)` / `fsync(file)` | 截断／落盘 |
| `close(handle)` | 释放（CP-17） |

**建议**：`dup`／`fdopen` 一类由实例能力表复制槽位实现（CP-14／CP-31），**不需要** provider 提供。
可异步化：上表全部。

### 9.2 `net` —— 网络（`_socket`、`select`）

| 槽位 | 语义 |
|---|---|
| `socket(family, type, proto)` | 返回 socket 能力 |
| `connect(sock, addr)` / `bind(sock, addr)` / `listen(sock, backlog)` / `accept(sock)` | 连接生命周期，`accept` 返回（socket 能力，地址） |
| `send(sock, buf)` / `recv(sock, n)` / `shutdown(sock, how)` | 收发 |
| `getsockname(sock)` / `getpeername(sock)` | 地址 |
| `getsockopt(sock, level, name)` / `setsockopt(sock, level, name, value)` | 选项 |
| `poll(fds, timeout_ns)` | 就绪集合——`select.select`／`select.poll` 的底座 |
| `close(handle)` | 释放（CP-17） |

地址**必须**是"地址族编号 ＋ 裸字节"，**禁止**在接口里出现 Python 对象。
可异步化：`poll` 之外全部；`poll` 是"等待"，其取消走 CP-27。

### 9.3 `proc` —— 进程（`posix`(fork/exec)、`_signal`）

| 槽位 | 语义 |
|---|---|
| `spawn(argv, env, fds)` | 创建进程，返回进程能力（fork＋exec 的合成；具体分解由 `SPEC-c-modules.md` 定） |
| `waitpid(proc, options)` | 等待并回收 |
| `kill(proc, sig)` | 发信号 |
| `pipe()` | 返回一对文件能力 |
| `close(handle)` | 释放（CP-17） |

信号**接收**（`_signal` 的处理分发）触碰 VM 状态，属不可异步化，具体形状待 `SPEC-c-modules.md` 提号。
可异步化：`spawn`／`waitpid`／`kill`／`pipe`。

### 9.4 `clock` —— 时钟（`time`）

| 槽位 | 语义 |
|---|---|
| `time_ns()` / `monotonic_ns()` | 墙钟／单调钟 |
| `sleep_ns(n)` | 可取消的睡眠（CP-27） |
| `resolution_ns()` | 时钟精度 |

时区规则**不**在本域：`zoneinfo` 是纯 Python，数据经 `fs` 读（`DESIGN.md` §9）。
可异步化：全部。

### 9.5 `random` —— 随机（`_random`、`os.urandom`）

| 槽位 | 语义 |
|---|---|
| `getrandom(buf)` | 填满熵源 |

**伪随机算法不在能力层**：`_random` 的实现与种子策略归 C 层模块（`SPEC-c-modules.md`），本域只给熵。
可异步化：是。

### 9.6 `env` —— 环境与身份（`posix`）

| 槽位 | 语义 |
|---|---|
| `getenv(name)` / `environ()` | 环境变量 |
| `getpid()` / `getuid()` / `geteuid()` / `getgid()` / `getegid()` | 身份 |
| `uname()` | 系统标识 |

受限 provider **只能**给受控值／常量（CP-24）。
可异步化：全部。

### 9.7 `tty` —— 终端（`termios`、`readline`、`_curses`）

| 槽位 | 语义 |
|---|---|
| `isatty(fd)` | 是否终端 |
| `getattr(fd)` / `setattr(fd, attrs)` | 终端属性 |
| `winsize(fd)` | 窗口大小 |
| `readline(prompt)` | 行读取（`readline` 的底座） |

可异步化：全部。

### 9.8 `locale` —— 区域与编码（`_locale`、`_codecs*`、`unicodedata`）

| 槽位 | 语义 |
|---|---|
| `setlocale(category, value)` / `localeconv()` | 区域设置与约定 |
| `table(name, offset, n)` | 编码／Unicode 数据表的读取入口 |

表**来源**（随附数据？生成？）**未决**，见 §13-7；本文件只要求存在一个受控读取入口，**禁止**在别处私自定表格式。
可异步化：全部。

### 9.9 `ipc` —— 进程内共享（`_thread`、`mmap`、`_posixshmem`）

| 槽位 | 语义 | 可异步化 |
|---|---|---|
| `mmap(...)` / `shm_open(name, flags, mode)` / `shm_unlink(name)` | 共享内存与映射 | 是 |
| `thread_*` | `_thread` 的线程创建与同步原语 | **否**——触碰 VM 对象，线程语义归 VM 侧（`DESIGN.md` §5） |

---

## 10. VM 侧 POSIX 适配层

- **CP-31** **实例能力表**：`fd` 的分配／释放／复制（含 `dup2`）**必须**在本层完成，`fd` 就是表下标（CP-14）。
- **CP-32** **路径解析器**：CP-21／CP-22 的实现位置；provider 侧**禁止**再度解释绝对路径。
- **CP-33** **机器错误 → Python 异常**的映射：provider 只给机器错误形状，映射表归 `SPEC-c-modules.md`；**临时假设**：按 `errno` 一一对应，本文件不钉具体类。
- **CP-34** **未实现 → Python 层**的呈现归 `SPEC-c-modules.md`（**临时假设**：一个独立异常，不复用 `OSError` 家族；CM- 未写出前不得据此实现）。
- **CP-35** 跨线程边界**必须**由本层把 VM 对象转成裸数据后再交给 worker；worker 返回后**必须**由本层把裸数据还原成 VM 侧值。**禁止**让 VM 对象进入 worker（CP-26）。
- **CP-36** 预算与记账在本层之内完成，**禁止**对外暴露（CP-8）。

---

## 11. 验收（可测性质）

| 编号 | 测试 |
|---|---|
| T-CP-1 | 域／槽位未实现时：创建实例成功，调用返回"未实现"（CP-2／CP-3／CP-5） |
| T-CP-2 | 宿主已实现但拒绝：返回真实机器错误（`PermissionError` 一类），与 T-CP-1 可区分（CP-6） |
| T-CP-3 | `fd` 是 `int`：`select.select([fd])`、`os.dup2`、`socket.fileno()` 照常工作（CP-14／CP-31） |
| T-CP-4 | 受限 provider 下 `..` 与符号链接无法跳出根能力（CP-21） |
| T-CP-5 | 受限 provider 下 `/proc`、`/dev`、`/sys` 报 `FileNotFoundError`（CP-23） |
| T-CP-6 | CI 静态检查：VM 核心与接口 crate 无平台依赖（`tests/ci/README.md` 第 3 项、CP-4、CP-12） |
| T-CP-7 | 两个实例的能力表互不可见；销毁任一实例后无残留句柄（CP-13／CP-19） |
| T-CP-8 | 未声明异步分类的域／槽位注册失败，不落默认值（CP-25） |
| T-CP-9 | 编译期断言：可异步化槽位的签名不含 VM 对象类型（CP-26） |
| T-CP-10 | 可异步化调用挂起时中断实例：调用以"已取消"返回，无后续副作用，脚本不悬挂（CP-19／CP-27／CP-28） |
| T-CP-11 | 同一脚本在真实机器 provider 与受限 provider 下**写法不变**（`DESIGN.md` §12 M5；判据见 `PLAN-milestones.md` §6 的 M5） |
| T-CP-12 | 关闭后再操作返回机器错误，且 `close` 幂等（CP-17／CP-18） |
| T-CP-13 | 能力接口 crate 内不存在真实机器实现（CP-12） |

---

## 12. 未决（引用 `DESIGN.md` §13）

- **§13-2** ABI 版本策略：能力契约必须与嵌入 API、宿主对象契约一起冻结 → 影响 CP-30 的版本字段形状
- **§13-4** 宿主自定义能力归属"可异步化／不可异步化"的判据与声明内容 → 影响 CP-25（本文件只定"必须声明、缺失即失败"）
- **§13-16** C ABI 函数数量预算（建议 ≤120）→ 这是本文件槽位总数的上限；逐模块补齐时必须按预算合并粒度
- **§13-10** 的**剩余开放**（`MS-9` 规范化容差、`MS-13` 基线语料范围下限）→ 决定 T-CP-11 何时真正可跑
- **§13-1** 宿主类型是否允许被脚本继承 → 影响宿主对象桥接（CP-15）在子类分派下的形状
- **§13-7** Unicode 版本与数据表来源 → 影响 §9.8 的 `table` 槽位

> 别处（`SPEC-c-abi.md`／`SPEC-c-modules.md`）写到一半发现需要新槽位时，按 §1 的归属规则回到本文件领号，
> **禁止**在别处定义能力函数——这是 `SPEC-INDEX.md` §4 划给本文件的唯一归属。
