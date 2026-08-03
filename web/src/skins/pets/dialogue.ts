import type { PetEventId } from './manifest'

export interface PetDialogueContext {
  projects: number
  backlog: number
  inProgress: number
  inReview: number
  done: number
  blocked: number
}

export const PET_DIALOGUE_CONFIG = Object.freeze({
  visibleDurationMs: 4_200,
  interactionLines: [
    '今天有风，适合推进。',
    '潮水退了又涨，Backlog 也是。',
    '海浪会把完成的工作温柔收藏。',
    '我在岸边替你看着任务。',
    '别急，先把下一步写清楚。',
    '任务多的时候，也要记得看看海。',
    '每一条活动记录，都是来时的脚印。',
    '换一个 Agent，航海图还在。',
    '小修小补是浪花，长期目标才是航线。',
    '如果卡住了，就从最小的验证开始。',
    '沙滩很宽，新任务总有地方落脚。',
    '我没有偷懒，只是在观察潮汐。',
  ],
})

const EVENT_LINES: Readonly<Record<PetEventId, readonly string[]>> = Object.freeze({
  'task-created': [
    '新任务落在沙滩上了。',
    '沙滩上多了一枚新的任务贝壳。',
    '新的航点已经记进任务地图。',
    '任务已写下，不会消失在聊天海雾里。',
  ],
  'task-started': [
    '它进入潮间带，开始推进。',
    '这项任务离开沙滩，正式启航。',
    '潮水动了，任务也开始向前。',
    '已经有人接住这条航线了。',
  ],
  'task-review': [
    '任务游到浅海，等待检查。',
    '实现告一段落，现在看看证据是否齐全。',
    '它正在浅海停泊，等一次认真验收。',
    '代码抵达评审区，别忘了核对验收项。',
  ],
  'task-done': [
    '完成啦，深海会把成果好好收藏。',
    '这条航线抵达终点，记录仍会留下。',
    '又一项工作沉淀进可追溯的深海。',
    '任务完成，海面上少了一块心事。',
  ],
  'task-blocked': [
    '有任务搁浅了，去看看阻塞原因吧。',
    '潮水推不动它，可能需要你的决定。',
    '这里出现了一处浅滩，阻塞信息已经标出。',
    '任务暂时停航，先处理最具体的障碍。',
  ],
  'sync-failed': [
    '海岸信号中断，项目同步需要处理。',
    'Watcher 没有顺利带回最新消息。',
    '项目扫描遇到风浪，界面里有详细原因。',
    '同步航标熄灭了，可以检查后安全重试。',
  ],
  'push-started': [
    '正在把航线交给 Agent…',
    '任务指令正在启航，请稍等。',
    '正在联络你选择的启动方式…',
    'Push 已开始，我会留意返回结果。',
  ],
  'push-succeeded': [
    'Agent 已收到任务，我会继续守望。',
    'Push 已完成，接下来留意执行记录。',
    '航线已经送出，任务事实仍保存在这里。',
    '交接成功，别忘了回来查看进展。',
  ],
  'push-failed': [
    'Agent 没有成功启航，可以安全重试。',
    '这次 Push 没有送达，先看看错误详情。',
    '启动方式遇到问题，任务本身没有丢失。',
    '交接失败了，修正配置后可以再次出发。',
  ],
})

export const dialogueForPetEvent = (event: PetEventId, index = 0): string => {
  const lines = EVENT_LINES[event]
  return lines[index % lines.length]
}

const contextualLines = (context: PetDialogueContext): string[] => {
  const lines: string[] = []
  if (context.blocked > 0) lines.push(`${context.blocked} 个任务在浅滩搁浅，先看看最具体的阻塞。`)
  if (context.inProgress > 0) lines.push(`潮间带里有 ${context.inProgress} 个任务正在推进。`)
  if (context.inReview > 0) lines.push(`浅海停着 ${context.inReview} 个任务，等待验收。`)
  if (context.backlog > 0) lines.push(`沙滩上还有 ${context.backlog} 个任务等待启航。`)
  if (context.done > 0) lines.push(`深海已经收藏了 ${context.done} 个完成任务。`)
  if (context.projects > 1) lines.push(`我正替你看着 ${context.projects} 个项目的海岸线。`)
  if (lines.length === 0) lines.push('海面很平静，暂时没有任务需要照看。')
  return lines
}

export const interactionDialogue = (index: number, context?: PetDialogueContext): string => {
  const lines = context
    ? [...contextualLines(context), ...PET_DIALOGUE_CONFIG.interactionLines]
    : PET_DIALOGUE_CONFIG.interactionLines
  return lines[index % lines.length]
}
