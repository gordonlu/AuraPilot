# AuraPilot 世界皮肤开发指南

本文说明如何为 AuraPilot 新建可维护、可恢复、不会影响任务协议的世界皮肤。现有的 `seascape` 与 `stellar` 是主要参考实现。

## 1. 边界与原则

世界皮肤负责视觉氛围、装饰动画和可选的互动角色，不负责业务行为。

- 不修改任务 Schema、状态流转、项目注册、Agent Profile 或 Push 语义。
- 不从运行时直接写入 Pinia 业务 Store、文件系统或 Tauri API。
- 皮肤加载失败时必须有可见反馈、可重试，并允许回退到经典界面。
- 动画必须可暂停、可释放，页面隐藏或切换皮肤后不得继续消耗资源。
- 所有业务内容仍需具备可读文本、键盘操作、可见焦点和足够对比度。
- 动效应遵守现有的 `prefers-reduced-motion` 降级规则。
- 通用参数集中配置；不要在组件、运行时和样式中重复散落同一组 magic numbers。

## 2. 现有架构

| 位置 | 职责 |
| --- | --- |
| `web/src/skins/worldSkin.ts` | 皮肤 ID、顺序、展示信息、持久化解析与公共配置 |
| `web/src/skins/WorldSkinHost.vue` | 懒加载运行时、生命周期接线、反馈、重试和事件分发 |
| `web/src/skins/runtime.ts` | 运行时契约与通用控制器 |
| `web/src/skins/<skin-id>/runtime.ts` | 某个动态世界的 PixiJS 场景实现 |
| `web/src/components/WorldSkinPickerModal.vue` | 皮肤选择器、预览和图标 |
| `web/src/style.css` | 主题 Token、业务界面皮肤化、选择器预览和降级动效 |
| `web/src/components/*BoardDecor.vue` | 可选的看板静态或 DOM/SVG 装饰层 |
| `web/src/skins/pets/` | 通用宠物清单、动画 Actor 和交互逻辑 |
| `web/public/pets/<pet-id>/` | 宠物清单与精灵图资源 |

`main.ts` 会在 Vue 挂载前从本地存储恢复皮肤，避免首屏闪烁；`App.vue` 负责保存选择、设置 `data-world-skin`，并在运行时失败时提供回退。

## 3. 最小目录结构

纯 CSS 皮肤不需要运行时目录。带动态世界或角色的皮肤建议使用：

```text
web/src/skins/aurora/
  runtime.ts
web/public/pets/aurora-guide/       # 可选
  pet.json
  spritesheet.webp
```

皮肤 ID 使用稳定的小写 kebab-case，例如 `aurora` 或 `deep-space`。ID 会被保存到本地偏好中，因此发布后不要随意改名。

## 4. 注册皮肤

### 4.1 更新统一定义

在 `web/src/skins/worldSkin.ts` 中同时完成以下事项：

1. 将 ID 加入 `WorldSkin` 联合类型。
2. 将 ID 加入 `WORLD_SKIN_ORDER`。
3. 在 `WORLD_SKIN_PRESENTATION` 中补充名称、操作文案、说明、动效说明和 Beta 状态。
4. 在 `resolveWorldSkin` 中接受该 ID。

不要只更新联合类型；否则皮肤可能能编译，却无法从本地存储恢复。

### 4.2 注册懒加载入口

如果皮肤有动态运行时，在 `WorldSkinHost.vue` 的 loader 中增加对应的动态 `import()`。运行时必须按需加载，不能进入应用时一次性加载所有 PixiJS 场景和资源。

如果皮肤没有动态运行时，应明确使用一个轻量空运行时，而不是让 loader 抛出“不支持”错误。

### 4.3 更新选择器

`WorldSkinPickerModal.vue` 会按照 `WORLD_SKIN_ORDER` 自动列出皮肤，但当前图标映射和 `style.css` 中的预览图是显式定义的。新皮肤必须补齐：

- 能代表主题的图标；
- `.world-skin-option.<skin-id> .world-skin-preview`；
- 选中、悬停、禁用和键盘焦点状态。

## 5. 主题样式

所有业务界面覆盖都应限定在：

```css
.app-shell[data-world-skin="aurora"] { /* theme tokens */ }
.app-shell[data-world-skin="aurora"] .task-card { /* scoped overrides */ }
```

优先覆盖现有语义 Token，而不是为每个组件硬编码互不相关的颜色：

```text
--bg-base       --bg-elev       --bg-sunken     --bg-hover
--border        --border-strong
--text-1        --text-2        --text-3
--accent        --accent-soft   --primary
--green         --yellow        --red
```

需要逐一检查普通、悬停、选中、禁用、零数量、警告和阻塞状态。装饰层默认使用 `pointer-events: none`，不得遮挡任务卡、按钮、滚动条或弹窗。

若主题需要改变看板语义标签或增加场景装饰，可参考 `SeascapeBoardDecor.vue`、`StellarBoardDecor.vue` 与 `BoardView.vue`。这一步是显式集成点，不会由注册表自动完成。

## 6. 动态运行时契约

运行时实现 `WorldSkinRuntime`：

```ts
interface WorldSkinRuntime {
  mount(container: HTMLElement, context: WorldSkinRuntimeContext): Promise<void>
  resize(viewport: WorldSkinViewport): void
  pause(reason: WorldSkinPauseReason): void
  resume(): void
  dispatch(event: WorldSkinRuntimeEvent): void
  dispose(): void
}
```

实现时必须满足：

- `mount` 在耗时初始化前后检查 `context.signal.aborted`。
- 初始化中途失败或取消时，清理已创建的 canvas、ticker、监听器和纹理。
- `resize` 使用宿主提供的尺寸与像素比例，不能假设固定窗口。
- `pause` 停止 ticker、定时器和非必要动画；`resume` 可安全重复调用。
- `dispatch` 只处理视觉反馈事件，例如 `pet-interact` 和 `pet-state`。
- `dispose` 幂等，切换皮肤后不留下 DOM、CSS 变量、监听器或 GPU 资源。
- 不吞掉异常。让控制器把错误转为可见的失败与重试状态。

参考 `seascape/runtime.ts` 和 `stellar/runtime.ts` 的 PixiJS 初始化、取消检查、宠物挂载与资源释放流程。动画参数应收敛到当前皮肤的配置对象中。

## 7. 可选宠物包

宠物资源放在 `web/public/pets/<pet-id>/`。最小 `pet.json`：

```json
{
  "id": "aurora-guide",
  "displayName": "极光向导",
  "description": "在极光世界中陪伴任务推进的角色。",
  "spritesheetPath": "spritesheet.webp"
}
```

当前 Hatch Pet 兼容图集为 1536 x 1872 像素，8 列 x 9 行，每格 192 x 208 像素；状态依次为 `idle`、`right`、`left`、`waving`、`jumping`、`failed`、`waiting`、`running`、`review`。资源必须保持透明背景，并在实际缩放尺寸下检查轮廓、闪烁、切格和边缘污染。

随后在 `web/src/skins/pets/actor.ts` 中增加集中式 Actor 配置，并从皮肤运行时挂载。角色称呼、交互提示和对话当前在 `WorldSkinHost.vue` 有主题分支；新增角色时必须一起更新，避免沿用其他世界的文案。

## 8. 可靠性与性能

- 首次资源加载不得阻塞 Vue UI；保留加载反馈所需的布局空间。
- 依赖 `WorldSkinController` 的统一超时、取消和“最新激活请求获胜”行为。
- 不启动无法取消的 Promise、无限定时器或脱离 Pixi ticker 的动画循环。
- 页面不可见、宿主卸载和皮肤切换时都要停止工作。
- 限制纹理尺寸、粒子数量和设备像素比成本；在窄窗口和高 DPI 屏幕验证。
- 对 WebGL 或资源加载失败提供经典皮肤回退，不让用户停留在永久加载状态。
- 不在皮肤代码中加入平台专用启动、剪贴板或 Agent CLI 逻辑。

## 9. 测试与验收

至少覆盖与改动直接相关的检查：

1. `worldSkin.ts`：ID 解析、持久化恢复和轮换顺序。
2. 选择器：选项可见、可选择、文案与 Beta 标记正确。
3. 动态运行时：成功挂载、超时、取消、切换、重复释放、暂停与恢复。
4. 宠物包：清单可解析、精灵图可加载、关键状态可播放。
5. 实际界面：桌面尺寸和一个窄视口，检查控制台、滚动、弹窗、切换与重试。
6. 可访问性：键盘操作、焦点、文字对比度和 reduced-motion 降级。

建议命令：

```sh
pnpm check
pnpm test -- web/src/dev-config.test.ts web/src/skins/runtime.test.ts
pnpm build
```

命令通过不等于动态世界已经真实可用。交付说明应区分：

- 已实现：代码和资源已接入。
- 单元验证：自动化测试覆盖了纯逻辑与生命周期。
- 运行时验证：在真实浏览器或 Tauri 窗口中观察过加载和交互。
- 恢复验证：实际触发过失败、重试、切换或回退路径。

## 10. Definition of Done

- [ ] 新 ID、顺序、展示信息、解析与 loader 均已注册。
- [ ] 选择器预览、图标和状态样式完整。
- [ ] 皮肤样式作用域正确，不污染 classic、light、brand 或 dark。
- [ ] 看板、项目页、阻塞页、弹窗与空状态均可读可操作。
- [ ] 动态运行时支持取消、暂停、恢复、缩放和幂等释放。
- [ ] 加载失败可见、可重试，并能回退。
- [ ] 宠物资源与对话已正确绑定（如适用）。
- [ ] 定向测试通过，并完成至少一次真实渲染检查。
- [ ] 交付报告没有把 fixture、模拟或仅编译通过描述成真实集成验证。
