/* ============================================================
   AuraPilot · Application Logic
   状态管理 + Mock 数据 + 视图渲染 + 交互
   ============================================================ */

// ============================================================
// 1. MOCK DATA — 模拟 .aurapilot/ 协议结构
// ============================================================
const PROJECTS = {
  'my-app': {
    name:'my-app', owner:'gordon', health:'green', sprint:'2026-W30',
    path:'/Users/gordon/dev/my-app',
    notes:'本周聚焦认证模块',
    tech:[['React','#61DAFB'],['TS','#3178C6'],['Node','#339933']],
    loc:'12.4k', branch:'feat/auth', ago:'2h ago', pr:1, agents:2,
    act:[1,2,3,2,4,1,2,3,4,2,3,4], cw:'28/wk'
  },
  'api-server': {
    name:'api-server', owner:'gordon', health:'yellow', sprint:'2026-W30',
    path:'/Users/gordon/dev/api-server',
    notes:'数据库迁移进行中',
    tech:[['Go','#00ADD8'],['Postgres','#4169E1'],['gRPC','#5B8D8A']],
    loc:'8.7k', branch:'chore/db', ago:'5h ago', pr:2, agents:1,
    act:[3,4,2,1,0,1,2,3,2,4,3,2], cw:'19/wk'
  },
  'web-dash': {
    name:'web-dash', owner:'gordon', health:'green', sprint:'2026-W30',
    path:'/Users/gordon/dev/web-dash',
    notes:'',
    tech:[['Vue','#42B883'],['Vite','#646CFF'],['TS','#3178C6']],
    loc:'6.1k', branch:'fix/mobile', ago:'1h ago', pr:0, agents:1,
    act:[0,1,1,2,3,2,1,0,2,3,4,3], cw:'12/wk'
  },
  'cli-tool': {
    name:'cli-tool', owner:'gordon', health:'red', sprint:'2026-W30',
    path:'/Users/gordon/dev/cli-tool',
    notes:'需要补充测试',
    tech:[['Rust','#DEA584'],['Clap','#FF6B35']],
    loc:'3.2k', branch:'docs/help', ago:'1d ago', pr:0, agents:0,
    act:[2,1,0,0,0,1,0,0,0,0,1,0], cw:'3/wk'
  },
  'docs-site': {
    name:'docs-site', owner:'gordon', health:'green', sprint:'2026-W30',
    path:'/Users/gordon/dev/docs-site',
    notes:'',
    tech:[['Astro','#FF5D01'],['MDX','#1A1A1E']],
    loc:'4.5k', branch:'main', ago:'3h ago', pr:0, agents:1,
    act:[1,1,2,1,2,3,2,1,2,3,2,1], cw:'8/wk'
  }
};

// tasks 按 .aurapilot/tasks/{state}/TASK-XXX.yaml 结构
let TASKS = [
  // --- backlog ---
  {id:'TASK-002',title:'修复设置页按钮错位',priority:'P2',type:'bug',created:'2026-07-25',
   state:'backlog',project:'my-app',assigned:'Claude Code',desc:'设置页在窄屏下保存按钮与取消按钮重叠',accept:['窄屏下按钮不重叠','间距符合设计稿'],blockers:[],ext:{estimate:'3h',tags:['ui','bug']}},
  {id:'TASK-003',title:'为构建脚本添加结构化日志',priority:'P1',type:'feature',created:'2026-07-24',
   state:'backlog',project:'my-app',assigned:null,desc:'当前构建输出难解析，需 JSON 结构化',accept:[],blockers:['上游 logging 库 API 未定稿（等待 v2 发布）'],ext:{}},
  {id:'TASK-005',title:'用户头像上传裁剪',priority:'P2',type:'feature',created:'2026-07-22',
   state:'backlog',project:'my-app',assigned:null,desc:'支持方形裁剪 + 预览',accept:['裁剪后 256x256','支持拖拽'],blockers:[],ext:{}},
  {id:'TASK-012',title:'添加 rate-limit 中间件',priority:'P1',type:'feature',created:'2026-07-23',
   state:'backlog',project:'api-server',assigned:null,desc:'按 IP 限流，可配置',accept:['默认 100 req/min','超限返回 429'],blockers:[],ext:{}},
  {id:'TASK-019',title:'CLI 支持 --json 输出',priority:'P2',type:'feature',created:'2026-07-21',
   state:'backlog',project:'cli-tool',assigned:null,desc:'脚本友好',accept:[],blockers:[],ext:{}},
  {id:'TASK-022',title:'首页 SEO meta 优化',priority:'P3',type:'chore',created:'2026-07-20',
   state:'backlog',project:'docs-site',assigned:null,desc:'',accept:[],blockers:[],ext:{}},
  {id:'TASK-023',title:'修复移动端布局溢出',priority:'P1',type:'bug',created:'2026-07-24',
   state:'backlog',project:'web-dash',assigned:null,desc:'图表在 iPhone SE 溢出',accept:[],blockers:['依赖的图表库尚未发布响应式补丁'],ext:{}},
  {id:'TASK-024',title:'添加 OpenTelemetry 追踪',priority:'P2',type:'feature',created:'2026-07-19',
   state:'backlog',project:'api-server',assigned:null,desc:'',accept:[],blockers:[],ext:{}},

  // --- in-progress ---
  {id:'TASK-000',title:'重构 auth 模块中间件',priority:'P0',type:'refactor',created:'2026-07-22',
   state:'in-progress',project:'my-app',assigned:'Claude Code',branch:'feat/auth-refactor',
   started:'2026-07-26T14:02:00+08:00',progress:60,
   desc:'当前 auth 中间件混杂了鉴权与日志逻辑，需拆分为独立的中间件链：\n- 解析 token\n- 校验权限\n- 注入请求上下文',
   accept:['单测覆盖三种 token 失效场景','性能回归 < 5ms p99'],
   blockers:[],ext:{estimate:'8h'},
   log:[
     {ts:'2026-07-26T14:02:00+08:00',msg:'任务被 Claude Code 领取，创建分支 feat/auth-refactor'},
     {ts:'2026-07-26T16:40:00+08:00',msg:'完成 token 解析中间件',model:'claude-3.5',lines:'+182'},
     {ts:'2026-07-27T09:15:00+08:00',msg:'权限校验完成，已提交 PR #142，等待 gordon 审阅'}
   ]},
  {id:'TASK-010',title:'迁移数据库至 PostgreSQL 16',priority:'P0',type:'chore',created:'2026-07-20',
   state:'in-progress',project:'api-server',assigned:'Codex',branch:'chore/db',
   started:'2026-07-25T10:00:00+08:00',progress:35,
   desc:'从 PG 14 升级到 16，利用逻辑复制',accept:['数据零丢失','停机 < 5min'],
   blockers:[],ext:{},
   log:[{ts:'2026-07-25T10:00:00+08:00',msg:'Codex 领取任务，开始迁移方案设计'}]},
  {id:'TASK-014',title:'补充集成测试覆盖',priority:'P1',type:'test',created:'2026-07-21',
   state:'in-progress',project:'api-server',assigned:'Codex',branch:'test/integration',
   started:'2026-07-24T08:30:00+08:00',progress:20,
   desc:'核心 API 路径集成测试',accept:['覆盖率 > 70%','CI 中运行 < 3min'],
   blockers:['测试数据库凭据过期','CI runner 磁盘满'],ext:{},
   log:[{ts:'2026-07-24T08:30:00+08:00',msg:'开始编写集成测试'}]},
  {id:'TASK-020',title:'文档站全文搜索',priority:'P2',type:'feature',created:'2026-07-22',
   state:'in-progress',project:'docs-site',assigned:'Cursor',branch:'feat/search',
   started:'2026-07-26T09:00:00+08:00',progress:45,
   desc:'集成 Pagefind',accept:['中文分词','< 100ms 响应'],blockers:[],ext:{},
   log:[{ts:'2026-07-26T09:00:00+08:00',msg:'Cursor 领取，集成 Pagefind'}]},
  {id:'TASK-021',title:'图表组件暗色主题适配',priority:'P2',type:'feature',created:'2026-07-23',
   state:'in-progress',project:'web-dash',assigned:'Gemini',branch:'feat/dark-charts',
   started:'2026-07-26T11:20:00+08:00',progress:80,
   desc:'ECharts 暗色 token 对接',accept:['与全局主题同步切换'],blockers:[],ext:{},
   log:[{ts:'2026-07-26T11:20:00+08:00',msg:'Gemini 领取'}]},

  // --- in-review ---
  {id:'TASK-009',title:'日志中间件 PR 待审',priority:'P1',type:'feature',created:'2026-07-19',
   state:'in-review',project:'api-server',assigned:'Codex',branch:'feat/log-mw',
   started:'2026-07-23T10:00:00+08:00',pr:138,waiting:'gordon',
   desc:'结构化请求日志',accept:['JSON 格式','可配置 level'],blockers:[],ext:{},
   log:[{ts:'2026-07-23T10:00:00+08:00',msg:'开始开发'},{ts:'2026-07-25T16:00:00+08:00',msg:'提交 PR #138'}]},
  {id:'TASK-016',title:'Vue3 升级至 3.4',priority:'P2',type:'chore',created:'2026-07-18',
   state:'in-review',project:'web-dash',assigned:'Gemini',branch:'chore/vue-up',
   started:'2026-07-24T14:00:00+08:00',pr:67,waiting:'gordon',
   desc:'',accept:['无 breaking change'],blockers:[],ext:{},
   log:[{ts:'2026-07-24T14:00:00+08:00',msg:'升级完成'}]},
  {id:'TASK-017',title:'README 重写',priority:'P3',type:'docs',created:'2026-07-17',
   state:'in-review',project:'cli-tool',assigned:'Claude Code',branch:'docs/readme',
   started:'2026-07-23T09:00:00+08:00',pr:12,waiting:'gordon',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-23T09:00:00+08:00',msg:'重写完成'}]},

  // --- done ---
  {id:'TASK-007',title:'升级 Express 至 v5',priority:'P1',type:'chore',created:'2026-07-15',
   state:'done',project:'my-app',assigned:'Claude Code',branch:'chore/express5',
   started:'2026-07-20T10:00:00+08:00',completed:'2026-07-24T15:00:00+08:00',commit:'a1b9f3c',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-24T15:00:00+08:00',msg:'PR 合并'}]},
  {id:'TASK-008',title:'清理废弃路由',priority:'P2',type:'refactor',created:'2026-07-14',
   state:'done',project:'my-app',assigned:'Claude Code',branch:'refactor/routes',
   started:'2026-07-18T10:00:00+08:00',completed:'2026-07-20T16:00:00+08:00',commit:'c4d5e6f',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-20T16:00:00+08:00',msg:'完成'}]},
  {id:'TASK-011',title:'协议违规示例',priority:'P3',type:'bug',created:'2026-07-13',
   state:'backlog',project:'cli-tool',assigned:null,parseFail:true,
   desc:'',accept:[],blockers:[],ext:{},log:[]},
  {id:'TASK-015',title:'健康检查端点',priority:'P1',type:'feature',created:'2026-07-16',
   state:'done',project:'api-server',assigned:'Codex',branch:'feat/health',
   started:'2026-07-19T10:00:00+08:00',completed:'2026-07-22T14:00:00+08:00',commit:'b7c8d9e',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-22T14:00:00+08:00',msg:'完成'}]},
  {id:'TASK-018',title:'补全 CLI 帮助文档',priority:'P3',type:'docs',created:'2026-07-12',
   state:'done',project:'cli-tool',assigned:'Claude Code',branch:'docs/help',
   started:'2026-07-15T10:00:00+08:00',completed:'2026-07-18T12:00:00+08:00',commit:'e9f0a1b',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-18T12:00:00+08:00',msg:'完成'}]},
  {id:'TASK-025',title:'Astro 升级至 4.10',priority:'P2',type:'chore',created:'2026-07-14',
   state:'done',project:'docs-site',assigned:'Cursor',branch:'chore/astro-up',
   started:'2026-07-17T10:00:00+08:00',completed:'2026-07-20T15:00:00+08:00',commit:'f2a3b4c',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-20T15:00:00+08:00',msg:'完成'}]},
  {id:'TASK-026',title:'侧边栏导航重构',priority:'P1',type:'refactor',created:'2026-07-13',
   state:'done',project:'docs-site',assigned:'Cursor',branch:'refactor/nav',
   started:'2026-07-16T10:00:00+08:00',completed:'2026-07-19T17:00:00+08:00',commit:'a5b6c7d',
   desc:'',accept:[],blockers:[],ext:{},log:[{ts:'2026-07-19T17:00:00+08:00',msg:'完成'}]}
];

// ============================================================
// 2. STORE — 全局状态
// ============================================================
const Store = {
  state:{
    view:'empty',           // empty | board | blocked | detail | newtask | wizard | settings
    lastBoardView:'board',  // board | blocked (返回看板用)
    theme:'dark',           // dark | light | system
    selectedTaskId:null,
    searchQuery:'',
    collapsedGroups:new Set(),
    collapsedCols:new Set(),
    settings:{
      defaultDir:'/Users/gordon/dev',
      cliPath:'/usr/local/bin/aurapilot',
      autoStart:true,
      theme:'dark',
      fontSize:13,
      density:'standard',
      lenientMode:true,
      schemaStrictness:40,
      gitignoreByDefault:false
    },
    wizardStep:1,
    toast:null,
    confirmAction:null      // {message, onConfirm}
  },

  listeners:[],

  get tasks(){return TASKS;},
  get projects(){return PROJECTS;},

  setState(patch){
    Object.assign(this.state, patch);
    this.notify();
  },
  subscribe(fn){this.listeners.push(fn);},
  notify(){this.listeners.forEach(fn=>fn(this.state));},

  // --- task operations ---
  getTask(id){return TASKS.find(t=>t.id===id);},
  getTasksByState(state){
    let tasks = TASKS.filter(t=>t.state===state);
    if(this.state.searchQuery){
      const q = this.state.searchQuery.toLowerCase();
      tasks = tasks.filter(t=>
        t.title.toLowerCase().includes(q) ||
        t.id.toLowerCase().includes(q) ||
        (t.assigned||'').toLowerCase().includes(q) ||
        t.project.toLowerCase().includes(q)
      );
    }
    return tasks;
  },
  getBlockedTasks(){
    return TASKS.filter(t=>t.blockers && t.blockers.length>0)
      .sort((a,b)=>this.blockDuration(b)-this.blockDuration(a));
  },
  blockDuration(t){
    if(t.started){
      const h = (Date.now()-new Date(t.started).getTime())/3600000;
      return h;
    }
    return 0;
  },
  nextTaskId(){
    const nums = TASKS.map(t=>parseInt(t.id.replace('TASK-','')));
    const max = Math.max(...nums, 0);
    return 'TASK-'+String(max+1).padStart(3,'0');
  },
  createTask(data){
    const id = this.nextTaskId();
    const task = {
      id, title:data.title, priority:data.priority, type:data.type,
      created:new Date().toISOString().slice(0,10),
      state:'backlog', project:data.project, assigned:null,
      desc:data.desc||'', accept:data.accept||[], blockers:[], ext:{}, log:[]
    };
    TASKS.push(task);
    this.notify();
    return task;
  },
  updateTask(id, patch){
    const t = this.getTask(id);
    if(t){Object.assign(t, patch); this.notify();}
  },
  deleteTask(id){
    const idx = TASKS.findIndex(t=>t.id===id);
    if(idx>=0){TASKS.splice(idx,1); this.notify();}
  },
  transitionTask(id, newState){
    const t = this.getTask(id);
    if(!t) return;
    const now = new Date().toISOString();
    const patch = {state:newState};
    if(newState==='in-progress' && !t.assigned){
      patch.assigned = 'You';
      patch.branch = 'task/'+t.id.toLowerCase();
      patch.started = now;
      patch.log = [...(t.log||[]), {ts:now, msg:'任务被领取，创建分支 '+patch.branch}];
    } else if(newState==='in-review'){
      patch.waiting = 'gordon';
      patch.log = [...(t.log||[]), {ts:now, msg:'提交审核'}];
    } else if(newState==='done'){
      patch.completed = now;
      patch.commit = Math.random().toString(16).slice(2,9);
      patch.log = [...(t.log||[]), {ts:now, msg:'任务完成归档'}];
    } else if(newState==='backlog'){
      // 重新打开
      delete patch.assigned; patch.assigned=null;
      patch.log = [...(t.log||[]), {ts:now, msg:'任务重新打开'}];
    }
    Object.assign(t, patch);
    this.notify();
    return t;
  },
  toggleGroup(key){
    const s = new Set(this.state.collapsedGroups);
    s.has(key)?s.delete(key):s.add(key);
    this.setState({collapsedGroups:s});
  },
  toggleCol(state){
    const s = new Set(this.state.collapsedCols);
    s.has(state)?s.delete(state):s.add(state);
    this.setState({collapsedCols:s});
  },
  showToast(msg, isErr){
    this.setState({toast:{msg, isErr}});
    clearTimeout(this._toastTimer);
    this._toastTimer = setTimeout(()=>this.setState({toast:null}), 2500);
  },
  requestConfirm(message, onConfirm){
    this.setState({confirmAction:{message, onConfirm}});
  },
  closeConfirm(){
    this.setState({confirmAction:null});
  }
};

// ============================================================
// 3. RENDER — 视图渲染
// ============================================================
const STATES = [
  {key:'backlog', name:'Backlog'},
  {key:'in-progress', name:'In Progress'},
  {key:'in-review', name:'In Review'},
  {key:'done', name:'Done'}
];

const ICONS = {
  robot:'<svg viewBox="0 0 24 24" fill="none"><rect x="4" y="8" width="16" height="11" rx="2.5" stroke="currentColor" stroke-width="1.6"/><circle cx="9" cy="13" r="1.2" fill="currentColor"/><circle cx="15" cy="13" r="1.2" fill="currentColor"/><path d="M9 8V6a3 3 0 0 1 6 0v2" stroke="currentColor" stroke-width="1.6"/></svg>',
  search:'<svg width="14" height="14" viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.8"/><path d="M20 20l-3-3" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
  folder:'<svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" stroke="currentColor" stroke-width="1.6"/></svg>',
  logo:(cls)=>`<svg class="${cls}" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="9" stroke="var(--accent)" stroke-width="1.6"/><path d="M12 3.5 L14.6 12 L12 20.5 L9.4 12 Z" fill="var(--accent)"/><circle cx="12" cy="12" r="1.6" fill="var(--bg-elev)"/></svg>`
};

function el(html){
  const tpl = document.createElement('template');
  tpl.innerHTML = html.trim();
  return tpl.content.firstElementChild;
}

function fmtDate(iso){
  if(!iso) return '—';
  const d = new Date(iso);
  return d.toLocaleString('zh-CN',{month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit'}).replace(/\//g,'-');
}
function fmtDuration(hours){
  if(hours < 1) return Math.round(hours*60)+' 分钟';
  if(hours < 24) return Math.round(hours)+' 小时';
  return Math.round(hours/24)+' 天';
}

// ---- Rail ----
function renderRail(){
  const taskCount = TASKS.length;
  const projCount = Object.keys(PROJECTS).length;
  const blockedCount = Store.getBlockedTasks().length;
  return `
    <aside class="rail">
      <div class="rail-brand">
        ${ICONS.logo('logo')}
        <div><b>AuraPilot</b><span>项目领航员</span></div>
      </div>
      <div class="rail-label">导航</div>
      <div class="rail-item ${Store.state.view==='board'?'active':''}" data-go="board">
        <span class="idx">1</span>看板
        <span class="cnt">${taskCount}</span>
      </div>
      <div class="rail-item ${Store.state.view==='blocked'?'active':''}" data-go="blocked">
        <span class="idx">2</span>阻塞聚焦
        ${blockedCount?`<span class="cnt" style="color:var(--red);border-color:rgba(248,81,73,.3)">${blockedCount}</span>`:''}
      </div>
      <div class="rail-item ${Store.state.view==='settings'?'active':''}" data-go="settings">
        <span class="idx">3</span>设置
      </div>
      <div class="rail-label">项目 (${projCount})</div>
      ${Object.values(PROJECTS).map(p=>`
        <div class="rail-item" data-go="board" data-filter-project="${p.name}">
          <span class="health dot-${p.health}" style="width:7px;height:7px;border-radius:50%;flex:0 0 7px"></span>
          ${p.name}
        </div>
      `).join('')}
      <div class="rail-spacer"></div>
      <div class="rail-foot">
        <button id="themeBtn">${Store.state.theme==='light'?'🌞 亮':'🌑 暗'}</button>
        <button id="newTaskBtn">+ 新建</button>
      </div>
    </aside>
  `;
}

// ---- Empty State ----
function renderEmpty(){
  return `
    <section class="view ${Store.state.view==='empty'?'active':''}" id="view-empty">
      <div class="empty">
        <div class="hero">
          ${ICONS.logo('logo-big')}
          <h1>AuraPilot</h1>
          <div class="sub">AI Coding 项目的领航员</div>
        </div>
        <div class="empty-cards">
          <div class="eco-card" data-action="add-project">
            <div class="ic"><svg width="22" height="22" viewBox="0 0 24 24" fill="none"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" stroke="currentColor" stroke-width="1.6"/><path d="M12 11v6M9 14h6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg></div>
            <h3>添加已有项目</h3>
            <p>选择本地已包含 <span class="mono">.aurapilot/</span> 的项目目录，立即接入。</p>
          </div>
          <div class="eco-card" data-action="init-project">
            <div class="ic"><svg width="22" height="22" viewBox="0 0 24 24" fill="none"><path d="M15 4h3a2 2 0 0 1 2 2v3M9 20H6a2 2 0 0 1-2-2v-3" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><path d="M19 9l-3.5 3.5a2 2 0 0 0 0 2.8l.2.2a2 2 0 0 0 2.8 0L21 12M5 15l3.5-3.5a2 2 0 0 0 0-2.8l-.2-.2a2 2 0 0 0-2.8 0L3 12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg></div>
            <h3>初始化新项目</h3>
            <p>选一个项目，自动生成 <span class="mono">.aurapilot/</span> 骨架与协议文件。</p>
          </div>
          <div class="eco-card" data-action="load-demo">
            <div class="ic"><svg width="22" height="22" viewBox="0 0 24 24" fill="none"><path d="M12 3l1.8 4.6L18 9l-4.2 1.4L12 15l-1.8-4.6L6 9l4.2-1.4L12 3Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M18 15l.9 2.3L21 18l-2.1.7L18 21l-.9-2.3L15 18l2.1-.7L18 15Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg></div>
            <h3>加载示例项目</h3>
            <p>一键 clone 内置 demo repo，零配置完整体验工作流。</p>
          </div>
        </div>
        <div class="flow">
          <div class="step"><div class="circle on">1</div>选项目</div>
          <div class="line"></div>
          <div class="step"><div class="circle">2</div>初始化</div>
          <div class="line"></div>
          <div class="step"><div class="circle">3</div>开始用</div>
        </div>
      </div>
    </section>
  `;
}

// ---- Board ----
function renderBoard(){
  const isLight = Store.state.theme==='light';
  const totalTasks = TASKS.length;
  const projCount = Object.keys(PROJECTS).length;

  const cols = STATES.map(s=>{
    const tasks = Store.getTasksByState(s.key);
    const collapsed = Store.state.collapsedCols.has(s.key);

    // group by project
    const groups = {};
    tasks.forEach(t=>{
      if(!groups[t.project]) groups[t.project] = [];
      groups[t.project].push(t);
    });

    const groupHtml = Object.entries(groups).map(([projName, projTasks])=>{
      const p = PROJECTS[projName];
      if(!p) return '';
      const gKey = s.key+'::'+projName;
      const collapsed = Store.state.collapsedGroups.has(gKey);
      const live = p.agents ? '<span class="live-pill"><span class="lpulse"></span>live</span>' : '';
      const tech = p.tech.map(t=>`<i><span class="tdot" style="background:${t[1]}"></span>${t[0]}</i>`).join('');
      const heat = p.act.map(l=>`<b class="l${l}"></b>`).join('');

      const taskHtml = projTasks.map(t=>renderTaskCard(t)).join('');

      return `
        <div class="group ${collapsed?'collapsed':''}" data-group="${gKey}">
          <div class="group-head" data-toggle-group="${gKey}">
            <span class="gchev">▾</span>
            <span class="gname">${projName}</span>
            <span class="health dot-${p.health}" title="健康度 ${p.health}"></span>
            ${live}
            <span class="gc">${projTasks.length} tasks</span>
          </div>
          <div class="group-sub">
            <span class="tech">${tech}</span>
            <span class="sm">${p.loc} LOC</span>
            <span class="sm">${p.branch}</span>
            <span class="sm">${p.ago}</span>
            ${p.pr?`<span class="sm pr">${p.pr} PR</span>`:''}
            ${p.agents?`<span class="sm ag">${p.agents} agent${p.agents>1?'s':''}</span>`:''}
            <span class="heat" title="近 12 天活跃度">${heat}<span class="sm" style="margin-left:4px">${p.cw}</span></span>
          </div>
          ${taskHtml}
        </div>
      `;
    }).join('');

    return `
      <div class="col ${collapsed?'collapsed':''}" data-col="${s.key}">
        <div class="col-head" data-toggle-col="${s.key}">
          <span class="name">${s.name}</span>
          <span class="cnt">${tasks.length}</span>
          <span class="chev">▾</span>
        </div>
        <div class="col-body">
          ${tasks.length===0?'<div class="col-empty">暂无任务</div>':groupHtml}
        </div>
      </div>
    `;
  }).join('');

  return `
    <section class="view ${Store.state.view==='board'?'active':''}" id="view-board">
      <div class="board">
        <div class="toolbar">
          <div class="brand">${ICONS.logo('logo')}AuraPilot</div>
          <span class="stat">${projCount} projects · ${totalTasks} tasks</span>
          <div class="search">
            ${ICONS.search}
            <input id="searchInput" placeholder="搜索任务、项目、Agent..." value="${Store.state.searchQuery}">
            <span class="kbd">/</span>
          </div>
          <div class="right">
            <div class="tabs">
              <button class="${Store.state.view==='board'?'on':''}" data-go="board">看板</button>
              <button class="${Store.state.view==='blocked'?'on':''}" data-go="blocked">阻塞</button>
            </div>
            <button class="btn btn-accent btn-sm" data-action="new-task">+ 新建任务</button>
          </div>
        </div>
        <div class="board-scroll">
          <div class="cols">${cols}</div>
        </div>
        <div class="statusbar">
          <span>最后刷新：刚刚</span>
          <span class="live"><span class="pulse"></span>实时监听中</span>
          <span class="spacer"></span>
          <span>${TASKS.filter(t=>t.state==='in-progress').length} 进行中 · ${Store.getBlockedTasks().length} 阻塞</span>
        </div>
      </div>
    </section>
  `;
}

function renderTaskCard(t){
  const isSelected = Store.state.selectedTaskId===t.id;
  const isBlocked = t.blockers && t.blockers.length>0;
  const hasExt = t.ext && Object.keys(t.ext).length>0;
  const extStr = hasExt ? Object.entries(t.ext).map(([k,v])=>`${k}=${Array.isArray(v)?'['+v.join(',')+']':v}`).join(' · ') : '';

  if(t.parseFail){
    return `
      <div class="task is-parsefail ${isSelected?'selected':''}" data-task="${t.id}">
        <div class="top"><span class="tid">${t.id}</span><span class="badge p3">P3</span><span class="proj">${t.project}</span></div>
        <div class="title">协议违规 · 点击查看</div>
        <div class="foot"><span class="chip"><span class="dot dot-yellow"></span>解析失败</span></div>
      </div>
    `;
  }

  const agentIcon = t.assigned ? `<span class="bot">${ICONS.robot}${t.assigned}</span>` : '<span class="bot">未分配</span>';
  const progBar = t.progress!=null ? `<span class="prog"><span style="width:${t.progress}%"></span></span>` : '';
  const blockedChip = isBlocked ? `<span class="chip"><span class="dot dot-red"></span>blockers: ${t.blockers.length}</span>` : '';
  const doneChip = t.state==='done' ? `<span class="chip"><span class="dot dot-green"></span>done ${t.completed?t.completed.slice(5,10):''}</span>` : '';
  const reviewInfo = t.state==='in-review' && t.waiting ? `<span class="bot">waiting: ${t.waiting}</span>` : '';

  return `
    <div class="task ${isBlocked?'is-blocked':''} ${isSelected?'selected':''}" data-task="${t.id}" data-dbl="task-detail">
      <div class="top">
        <span class="tid">${t.id}</span>
        <span class="badge ${t.priority.toLowerCase()}">${t.priority}</span>
        <span class="proj">${t.project}</span>
      </div>
      <div class="title">${escapeHtml(t.title)}</div>
      <div class="foot">
        ${agentIcon}${progBar}
        ${blockedChip}${doneChip}${reviewInfo}
      </div>
      ${hasExt?`<span class="ext" title="扩展字段：${escapeHtml(extStr)}">ℹ</span>`:''}
    </div>
  `;
}

// ---- Blocked Focus ----
function renderBlocked(){
  const blocked = Store.getBlockedTasks();
  return `
    <section class="view theme-blocked ${Store.state.view==='blocked'?'active':''}" id="view-blocked">
      <div class="blocked">
        <div class="blocked-head">
          <h2><span class="w">⚠</span> 阻塞聚焦 · ${blocked.length} 个任务需要介入</h2>
          <button class="btn btn-sm" data-go="board" style="margin-left:auto">← 返回看板</button>
        </div>
        <div class="blocked-list">
          ${blocked.length===0?'<div class="col-empty" style="padding:40px">暂无阻塞任务 🎉</div>':blocked.map(t=>{
            const dur = Store.blockDuration(t);
            return `
              <div class="brow" data-task="${t.id}" data-dbl="task-detail">
                <div class="left">
                  <div class="meta"><b>${t.project}</b> · ${t.id}</div>
                  <div class="ttl">${escapeHtml(t.title)}</div>
                </div>
                <div class="mid">blockers: ${escapeHtml(t.blockers.join(' · '))}</div>
                <div class="right">
                  <span class="dur">阻塞 ${fmtDuration(dur)}</span>
                  <button class="btn btn-sm" data-task-detail="${t.id}">查看详情</button>
                </div>
              </div>
            `;
          }).join('')}
        </div>
      </div>
    </section>
  `;
}

// ---- Task Detail Modal ----
function renderDetail(){
  const t = Store.getTask(Store.state.selectedTaskId);
  if(!t) return '<section class="view"></section>';
  const stateLabel = STATES.find(s=>s.key===t.state)?.name || t.state;

  const logHtml = (t.log||[]).map(l=>{
    const hasExtLog = Object.keys(l).some(k=>!['ts','msg'].includes(k));
    const extInfo = hasExtLog ? `<span class="ext-ic" title="扩展字段：${escapeHtml(Object.entries(l).filter(([k])=>!['ts','msg'].includes(k)).map(([k,v])=>`${k}=${v}`).join(' · '))}">ℹ</span>` : '';
    return `<div class="tl"><div class="ts">${fmtDate(l.ts)}</div><div class="msg">${escapeHtml(l.msg)}${extInfo}</div></div>`;
  }).join('');

  return `
    <section class="view ${Store.state.view==='detail'?'active':''}" id="view-detail">
      <div class="modal-layer" data-close-layer="1">
        <div class="modal detail" data-stop="1">
          <div class="modal-head">
            <span class="tid">${t.id}</span>
            <span class="badge ${t.priority.toLowerCase()}">${t.priority}</span>
            <span class="chip">${t.project} · ${stateLabel}</span>
            <button class="x" data-close="1">×</button>
          </div>
          <div class="modal-body">
            <input class="title-input" id="detail-title" value="${escapeHtml(t.title)}">
            <div class="field-row">
              <div class="field"><label>优先级</label>
                <select id="detail-priority">
                  ${['P0','P1','P2','P3'].map(p=>`<option ${p===t.priority?'selected':''}>${p}</option>`).join('')}
                </select></div>
              <div class="field"><label>类型</label>
                <select id="detail-type">
                  ${['feature','bug','refactor','docs','test','chore'].map(p=>`<option ${p===t.type?'selected':''}>${p}</option>`).join('')}
                </select></div>
              <div class="field"><label>状态（只读）</label>
                <input type="text" value="${stateLabel}" disabled></div>
            </div>
            <div class="field">
              <label>描述</label>
              <textarea id="detail-desc">${escapeHtml(t.desc||'')}</textarea>
            </div>
            <div class="field">
              <label>验收标准</label>
              <div class="crit-list" id="crit-list">
                ${(t.accept||[]).map((c,i)=>`
                  <div class="crit" data-crit="${i}">
                    <span class="ck ${t.state==='done'?'':'todo'}">${t.state==='done'?'✓':'○'}</span>
                    <span class="txt">${escapeHtml(c)}</span>
                    <button class="rm" data-rm-crit="${i}">×</button>
                  </div>
                `).join('')}
                <button class="crit-add" data-add-crit="1">+ 添加验收标准</button>
              </div>
            </div>
            <div class="field">
              <label>元数据（只读）</label>
              <div class="meta-grid">
                <div class="mi"><span class="k">assigned</span><span class="v ${t.assigned?'':'empty'}">${t.assigned||'—'}</span></div>
                <div class="mi"><span class="k">branch</span><span class="v ${t.branch?'':'empty'}">${t.branch||'—'}</span></div>
                <div class="mi"><span class="k">started</span><span class="v">${fmtDate(t.started)}</span></div>
                <div class="mi"><span class="k">pr</span><span class="v ${t.pr?'':'empty'}">${t.pr?'#'+t.pr:'—'}</span></div>
                <div class="mi"><span class="k">waiting</span><span class="v ${t.waiting?'':'empty'}">${t.waiting||'—'}</span></div>
                <div class="mi"><span class="k">completed</span><span class="v">${fmtDate(t.completed)}</span></div>
                <div class="mi"><span class="k">commit</span><span class="v ${t.commit?'':'empty'}">${t.commit||'—'}</span></div>
                <div class="mi"><span class="k">blockers</span><span class="v ${t.blockers&&t.blockers.length?'':''}" style="${t.blockers&&t.blockers.length?'color:var(--red)':''}">${t.blockers&&t.blockers.length?t.blockers.length+' 项':'无'}</span></div>
              </div>
            </div>
            ${t.log&&t.log.length?`
            <div class="field">
              <label>进度日志</label>
              <div class="timeline">${logHtml}</div>
            </div>`:''}
          </div>
          <div class="modal-foot">
            ${t.state==='backlog'?`<button class="btn btn-accent btn-sm" data-transition="in-progress">领任务</button>`:''}
            ${t.state==='in-progress'?`<button class="btn btn-accent btn-sm" data-transition="in-review">提交审核</button>`:''}
            ${t.state==='in-review'?`<button class="btn btn-accent btn-sm" data-transition="done">完成</button>`:''}
            ${t.state!=='backlog'?`<button class="btn btn-sm" data-transition="backlog">重新打开</button>`:''}
            <div class="right">
              <button class="btn btn-sm" data-save="1">保存</button>
              <button class="btn btn-sm" data-close="1">取消</button>
              <button class="btn btn-danger btn-sm" data-delete="1">删除</button>
            </div>
          </div>
        </div>
      </div>
    </section>
  `;
}

// ---- New Task Modal ----
function renderNewTask(){
  const projOptions = Object.keys(PROJECTS).map(p=>`<option>${p}</option>`).join('');
  return `
    <section class="view ${Store.state.view==='newtask'?'active':''}" id="view-newtask">
      <div class="modal-layer" data-close-layer="1">
        <div class="modal newt" data-stop="1">
          <div class="modal-head"><span class="tid" style="color:var(--text-1)">新建任务</span><button class="x" data-close="1">×</button></div>
          <div class="modal-body">
            <div class="field"><label>项目 <span class="req">*</span></label>
              <select id="nt-project">${projOptions}</select></div>
            <div class="field"><label>标题 <span class="req">*</span> <span style="color:var(--text-3);font-weight:500">1–120 字符</span></label>
              <input type="text" id="nt-title" placeholder="简明描述要做什么" maxlength="120"></div>
            <div class="field"><label>优先级</label>
              <div class="seg" id="nt-priority">
                ${['P0','P1','P2','P3'].map((p,i)=>`<button class="${i===1?'on':''}">${p}</button>`).join('')}
              </div></div>
            <div class="field"><label>类型</label>
              <div class="seg" id="nt-type">
                ${['feature','bug','refactor','docs','test','chore'].map((p,i)=>`<button class="${i===0?'on':''}">${p}</button>`).join('')}
              </div></div>
            <div class="field"><label>描述 <span style="color:var(--text-3);font-weight:500">（可选）</span></label>
              <textarea id="nt-desc" placeholder="补充上下文、关联 issue 等"></textarea></div>
            <div class="field"><label>验收标准 <span style="color:var(--text-3);font-weight:500">（可选）</span></label>
              <div class="crit-list" id="nt-crit">
                <button class="crit-add" data-nt-add-crit="1">+ 添加验收标准</button>
              </div></div>
          </div>
          <div class="modal-foot">
            <button class="btn btn-sm" data-close="1">取消</button>
            <div class="right"><button class="btn btn-accent btn-sm" data-create="1">创建</button></div>
          </div>
        </div>
      </div>
    </section>
  `;
}

// ---- Init Wizard ----
function renderWizard(){
  const step = Store.state.wizardStep;
  const projName = Store.state.wizardData?.name || '';
  return `
    <section class="view ${Store.state.view==='wizard'?'active':''}" id="view-wizard">
      <div class="modal-layer" data-close-layer="1">
        <div class="modal wizard" data-stop="1">
          <div class="modal-head"><span class="tid" style="color:var(--text-1)">初始化新项目</span><button class="x" data-close="1">×</button></div>
          <div class="stepper">
            <div class="s ${step===1?'on':''} ${step>1?'done':''}"><span class="n">${step>1?'✓':'1'}</span>选择路径</div>
            <div class="ln"></div>
            <div class="s ${step===2?'on':''} ${step>2?'done':''}"><span class="n">${step>2?'✓':'2'}</span>填写信息</div>
            <div class="ln"></div>
            <div class="s ${step===3?'on':''}"><span class="n">3</span>确认</div>
          </div>
          <div class="modal-body">
            ${step===1?`
              <div class="field"><label>项目路径</label>
                <div class="search" style="max-width:none">
                  ${ICONS.folder}
                  <input id="wz-path" value="/Users/gordon/dev/${projName||'aura-demo'}" placeholder="选择本地项目目录">
                </div>
              </div>
              <div class="note" style="background:var(--accent-soft);border:1px solid var(--accent-soft-2);border-radius:var(--r-md);padding:var(--sp-3) var(--sp-4);color:var(--text-1);font-size:var(--fs-sm)">
                将在所选目录下创建 <span class="mono">.aurapilot/</span> 完整骨架，含 AGENTS.md、project.yaml、schema.json 及 4 个状态目录。
              </div>
            `:''}
            ${step===2?`
              <div class="field"><label>项目名 <span class="req">*</span></label><input type="text" id="wz-name" value="${projName||'aura-demo'}"></div>
              <div class="field"><label>所有者 <span class="req">*</span></label><input type="text" id="wz-owner" value="gordon"></div>
              <div class="field-row">
                <div class="field"><label>健康度</label>
                  <div class="seg" id="wz-health">
                    <button class="on">green</button><button>yellow</button><button>red</button>
                  </div></div>
                <div class="field" style="grid-column:span 1"><label>备注（可选）</label><input type="text" id="wz-notes" placeholder="—"></div>
              </div>
            `:''}
            ${step===3?`
              <div class="meta-grid">
                <div class="mi"><span class="k">路径</span><span class="v" id="conf-path">/Users/gordon/dev/${projName||'aura-demo'}</span></div>
                <div class="mi"><span class="k">项目名</span><span class="v">${projName||'aura-demo'}</span></div>
                <div class="mi"><span class="k">所有者</span><span class="v">gordon</span></div>
                <div class="mi"><span class="k">健康度</span><span class="v">green</span></div>
                <div class="mi" style="grid-column:span 2"><span class="k">将创建</span><span class="v">.aurapilot/ + AGENTS.md + project.yaml + schema.json + tasks/{backlog,in-progress,in-review,done}/</span></div>
              </div>
              <div class="field">
                <label>Git 选项</label>
                <div class="set-row" style="border:none;padding:0">
                  <div class="lab"><b>加入 .gitignore</b><span>状态文件不被 git 追踪（默认不勾选）</span></div>
                  <div class="ctrl"><div class="toggle" id="wz-gitignore"></div></div>
                </div>
              </div>
            `:''}
          </div>
          <div class="modal-foot">
            <button class="btn btn-sm" data-wz-prev="1" ${step===1?'disabled':''}>上一步</button>
            <div class="right"><button class="btn btn-accent btn-sm" data-wz-next="1">${step===3?'完成':'下一步'}</button></div>
          </div>
        </div>
      </div>
    </section>
  `;
}

// ---- Settings ----
function renderSettings(){
  const s = Store.state.settings;
  const tab = Store.state.settingsTab || 'general';
  const projList = Object.values(PROJECTS).map(p=>`
    <div class="proj-row">
      <span class="ph dot-${p.health}"></span>
      <div>
        <div class="pn">${p.name}</div>
        <div class="pp">${p.path}</div>
      </div>
      <button class="btn btn-danger btn-sm" data-remove-project="${p.name}" style="margin-left:auto">移除</button>
    </div>
  `).join('');

  return `
    <section class="view ${Store.state.view==='settings'?'active':''}" id="view-settings">
      <div class="settings">
        <div class="set-nav">
          ${['general','appearance','protocol','advanced'].map(t=>`
            <div class="si ${tab===t?'on':''}" data-set-tab="${t}">${({general:'常规',appearance:'外观',protocol:'协议',advanced:'高级'})[t]}</div>
          `).join('')}
        </div>
        <div class="set-body">
          ${tab==='general'?`
            <h2>常规</h2>
            <div class="set-row">
              <div class="lab"><b>默认项目目录</b><span>新建/扫描项目时的起始路径</span></div>
              <div class="ctrl"><input type="text" value="${s.defaultDir}" data-set="defaultDir"></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>CLI 路径</b><span>aurapilot 命令行可执行文件位置</span></div>
              <div class="ctrl"><input type="text" value="${s.cliPath}" data-set="cliPath"></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>开机自启动</b><span>登录系统后自动在后台运行</span></div>
              <div class="ctrl"><div class="toggle ${s.autoStart?'on':''}" data-set-toggle="autoStart"></div></div>
            </div>
            <h2 style="margin-top:var(--sp-7)">已注册项目</h2>
            <div class="proj-list">${projList}</div>
          `:''}
          ${tab==='appearance'?`
            <h2>外观</h2>
            <div class="set-row">
              <div class="lab"><b>主题</b><span>亮色 / 暗色 / 跟随系统</span></div>
              <div class="ctrl"><div class="seg-sm" id="seg-theme">
                <button class="${s.theme==='light'?'on':''}" data-theme-val="light">亮</button>
                <button class="${s.theme==='dark'?'on':''}" data-theme-val="dark">暗</button>
                <button class="${s.theme==='system'?'on':''}" data-theme-val="system">系统</button>
              </div></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>字体大小</b><span>界面基础字号</span></div>
              <div class="ctrl"><input type="range" min="12" max="16" value="${s.fontSize}" class="slider" data-set-num="fontSize"><span class="mono" style="color:var(--text-2);font-size:12px;margin-left:8px">${s.fontSize}px</span></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>信息密度</b><span>紧凑 / 标准 / 宽松</span></div>
              <div class="ctrl"><div class="seg-sm" id="seg-density">
                <button class="${s.density==='compact'?'on':''}" data-density="compact">紧凑</button>
                <button class="${s.density==='standard'?'on':''}" data-density="standard">标准</button>
                <button class="${s.density==='comfortable'?'on':''}" data-density="comfortable">宽松</button>
              </div></div>
            </div>
          `:''}
          ${tab==='protocol'?`
            <h2>协议</h2>
            <div class="set-row">
              <div class="lab"><b>宽松模式</b><span>缺失必填字段时警告但不拒绝（默认开）</span></div>
              <div class="ctrl"><div class="toggle ${s.lenientMode?'on':''}" data-set-toggle="lenientMode"></div></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>schema 校验严格度</b><span>滑块越靠右越严格</span></div>
              <div class="ctrl"><input type="range" min="0" max="100" value="${s.schemaStrictness}" class="slider" data-set-num="schemaStrictness"><span class="mono" style="color:var(--text-2);font-size:12px;margin-left:8px">${s.schemaStrictness}</span></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>默认加入 .gitignore</b><span>初始化时是否默认勾选「不提交到 git」</span></div>
              <div class="ctrl"><div class="toggle ${s.gitignoreByDefault?'on':''}" data-set-toggle="gitignoreByDefault"></div></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>协议违规</b><span>查看解析失败的任务清单</span></div>
              <div class="ctrl"><button class="btn btn-sm" data-show-violations="1">查看违规面板</button></div>
            </div>
          `:''}
          ${tab==='advanced'?`
            <h2>高级</h2>
            <div class="set-row">
              <div class="lab"><b>导出配置</b><span>导出当前设置与项目注册表</span></div>
              <div class="ctrl"><button class="btn btn-sm" data-export="1">导出…</button></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>重置</b><span>恢复全部设置为默认值</span></div>
              <div class="ctrl"><button class="btn btn-danger btn-sm" data-reset="1">重置</button></div>
            </div>
            <div class="set-row">
              <div class="lab"><b>关于</b><span>AuraPilot v1.0.0 · Tauri 2.0</span></div>
              <div class="ctrl"><span class="mono" style="color:var(--text-3);font-size:12px">build 2026.07</span></div>
            </div>
          `:''}
        </div>
      </div>
    </section>
  `;
}

// ---- Toast ----
function renderToast(){
  if(!Store.state.toast) return '';
  const t = Store.state.toast;
  return `<div class="toast ${t.isErr?'err':''} show"><span class="ok">${t.isErr?'✕':'✓'}</span>${escapeHtml(t.msg)}</div>`;
}

// ---- Confirm Dialog ----
function renderConfirm(){
  if(!Store.state.confirmAction) return '';
  const c = Store.state.confirmAction;
  return `
    <div class="confirm-layer">
      <div class="confirm">
        <div class="warn-ic">⚠</div>
        <h3>确认操作</h3>
        <p>${escapeHtml(c.message)}</p>
        <div class="acts">
          <button class="btn btn-sm" data-confirm-cancel="1">取消</button>
          <button class="btn btn-danger btn-sm" data-confirm-ok="1">确认删除</button>
        </div>
      </div>
    </div>
  `;
}

function escapeHtml(s){
  if(s==null) return '';
  return String(s).replace(/[&<>"']/g, c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}

// ============================================================
// 4. APP — 主渲染与事件
// ============================================================
function render(){
  const views = [
    renderEmpty(),
    renderBoard(),
    renderBlocked(),
    Store.state.view==='detail' ? renderDetail() : '<section class="view"></section>',
    Store.state.view==='newtask' ? renderNewTask() : '<section class="view"></section>',
    Store.state.view==='wizard' ? renderWizard() : '<section class="view"></section>',
    renderSettings()
  ];

  const stageHtml = views.join('\n');
  const toastHtml = renderToast();
  const confirmHtml = renderConfirm();

  document.getElementById('app-root').innerHTML = `
    ${renderRail()}
    <div class="stage-wrap">
      <div class="stage">${stageHtml}</div>
    </div>
    ${toastHtml}
    ${confirmHtml}
  `;

  applyTheme();
  attachEvents();
}

function applyTheme(){
  const t = Store.state.theme;
  const resolved = t==='system'
    ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
    : t;
  document.documentElement.setAttribute('data-theme', resolved);
  const fs = Store.state.settings.fontSize;
  document.documentElement.style.setProperty('--fs-base', fs+'px');
}

// ---- view switching ----
function setView(v){
  const cur = Store.state.view;
  const modalViews = ['detail','newtask','wizard'];
  if(cur && !modalViews.includes(cur)){
    Store.state.lastBoardView = cur;
  }
  Store.setState({view:v});
}

function openTaskDetail(id){
  Store.setState({selectedTaskId:id, view:'detail'});
}

// ---- event delegation ----
function attachEvents(){
  const root = document.getElementById('app-root');

  // rail nav
  root.querySelectorAll('[data-go]').forEach(el=>{
    el.addEventListener('click', e=>{
      const v = el.dataset.go;
      if(v==='board') Store.state.lastBoardView='board';
      setView(v);
    });
  });

  // theme button
  const themeBtn = document.getElementById('themeBtn');
  if(themeBtn) themeBtn.addEventListener('click', ()=>{
    const cur = Store.state.theme;
    const next = cur==='light'?'dark':'light';
    Store.setState({theme:next});
    Store.state.settings.theme = next;
  });

  // new task button (rail)
  const newTaskBtn = document.getElementById('newTaskBtn');
  if(newTaskBtn) newTaskBtn.addEventListener('click', ()=>setView('newtask'));

  // empty state cards
  root.querySelectorAll('[data-action]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const a = el.dataset.action;
      if(a==='add-project') Store.showToast('请使用设置页添加项目目录');
      else if(a==='init-project') setView('wizard');
      else if(a==='load-demo'){ Store.showToast('示例项目已加载（mock）'); setView('board'); }
    });
  });

  // new task action
  root.querySelectorAll('[data-action="new-task"]').forEach(el=>{
    el.addEventListener('click', ()=>setView('newtask'));
  });

  // task card click (select) + dblclick (detail)
  root.querySelectorAll('[data-task]').forEach(el=>{
    el.addEventListener('click', e=>{
      if(e.target.closest('[data-task-detail]')||e.target.closest('.btn')) return;
      const id = el.dataset.task;
      Store.setState({selectedTaskId: Store.state.selectedTaskId===id ? null : id});
    });
    el.addEventListener('dblclick', e=>{
      const id = el.dataset.task;
      openTaskDetail(id);
    });
  });

  // task detail button (blocked view)
  root.querySelectorAll('[data-task-detail]').forEach(el=>{
    el.addEventListener('click', e=>{
      e.stopPropagation();
      openTaskDetail(el.dataset.taskDetail);
    });
  });

  // col/group toggle
  root.querySelectorAll('[data-toggle-col]').forEach(el=>{
    el.addEventListener('click', ()=>Store.toggleCol(el.dataset.toggleCol));
  });
  root.querySelectorAll('[data-toggle-group]').forEach(el=>{
    el.addEventListener('click', ()=>Store.toggleGroup(el.dataset.toggleGroup));
  });

  // search
  const searchInput = document.getElementById('searchInput');
  if(searchInput){
    searchInput.addEventListener('input', e=>{
      Store.setState({searchQuery:e.target.value});
    });
  }

  // modal close
  root.querySelectorAll('[data-close]').forEach(el=>{
    el.addEventListener('click', ()=>closeModal());
  });
  root.querySelectorAll('[data-close-layer]').forEach(el=>{
    el.addEventListener('click', e=>{ if(e.target===el) closeModal(); });
  });

  // detail: save / delete / transition
  root.querySelectorAll('[data-save]').forEach(el=>{
    el.addEventListener('click', ()=>saveDetail());
  });
  root.querySelectorAll('[data-delete]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const id = Store.state.selectedTaskId;
      Store.requestConfirm(`确认删除任务 ${id}？此操作不可撤销（将执行 git rm）。`, ()=>{
        Store.deleteTask(id);
        Store.closeConfirm();
        closeModal();
        Store.showToast('已删除 '+id);
      });
    });
  });
  root.querySelectorAll('[data-transition]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const id = Store.state.selectedTaskId;
      const newState = el.dataset.transition;
      const t = Store.transitionTask(id, newState);
      Store.showToast(`${id} → ${newState}`);
      if(newState!=='backlog') closeModal();
    });
  });

  // detail: add/remove criteria
  root.querySelectorAll('[data-add-crit]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const id = Store.state.selectedTaskId;
      const t = Store.getTask(id);
      const text = prompt('输入验收标准：');
      if(text && text.trim()){
        Store.updateTask(id, {accept:[...(t.accept||[]), text.trim()]});
      }
    });
  });
  root.querySelectorAll('[data-rm-crit]').forEach(el=>{
    el.addEventListener('click', e=>{
      e.stopPropagation();
      const idx = parseInt(el.dataset.rmCrit);
      const id = Store.state.selectedTaskId;
      const t = Store.getTask(id);
      const accept = [...(t.accept||[])];
      accept.splice(idx,1);
      Store.updateTask(id, {accept});
    });
  });

  // new task: create
  root.querySelectorAll('[data-create]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const project = document.getElementById('nt-project').value;
      const title = document.getElementById('nt-title').value.trim();
      if(!title){ Store.showToast('请填写标题', true); return; }
      const priority = document.querySelector('#nt-priority .on')?.textContent || 'P1';
      const type = document.querySelector('#nt-type .on')?.textContent || 'feature';
      const desc = document.getElementById('nt-desc').value;
      const accept = [...document.querySelectorAll('#nt-crit .crit .txt')].map(e=>e.textContent);
      const task = Store.createTask({project, title, priority, type, desc, accept});
      closeModal();
      Store.showToast(`已创建 ${task.id} 于 ${project}/backlog/`);
    });
  });

  // new task: add criteria
  root.querySelectorAll('[data-nt-add-crit]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const list = document.getElementById('nt-crit');
      const addBtn = list.querySelector('.crit-add');
      const div = document.createElement('div');
      div.className = 'crit';
      div.innerHTML = `<span class="ck todo">○</span><input placeholder="验收标准..."><button class="rm" type="button">×</button>`;
      addBtn.before(div);
      div.querySelector('input').focus();
      div.querySelector('.rm').addEventListener('click', ()=>div.remove());
    });
  });

  // wizard
  root.querySelectorAll('[data-wz-next]').forEach(el=>{
    el.addEventListener('click', e=>{
      e.stopPropagation();
      const step = Store.state.wizardStep;
      if(step===1){
        // capture path
        const pathInput = document.getElementById('wz-path');
        if(pathInput) Store.state.wizardData = Store.state.wizardData||{};
        if(pathInput) Store.state.wizardData.path = pathInput.value;
      }
      if(step===2){
        const name = document.getElementById('wz-name')?.value.trim();
        if(!name){ Store.showToast('请填写项目名', true); return; }
        Store.state.wizardData = Store.state.wizardData||{};
        Store.state.wizardData.name = name;
        Store.state.wizardData.owner = document.getElementById('wz-owner')?.value;
        Store.state.wizardData.health = document.querySelector('#wz-health .on')?.textContent;
      }
      if(step===3){
        // finish
        const name = Store.state.wizardData?.name || 'aura-demo';
        Store.showToast(`已初始化 ${name} 的 .aurapilot/`);
        Store.setState({wizardStep:1, wizardData:null});
        closeModal();
        setView('board');
        return;
      }
      Store.setState({wizardStep:step+1});
    });
  });
  root.querySelectorAll('[data-wz-prev]').forEach(el=>{
    el.addEventListener('click', e=>{
      e.stopPropagation();
      if(Store.state.wizardStep>1) Store.setState({wizardStep:Store.state.wizardStep-1});
    });
  });

  // settings tabs
  root.querySelectorAll('[data-set-tab]').forEach(el=>{
    el.addEventListener('click', ()=>{
      Store.setState({settingsTab:el.dataset.setTab});
    });
  });

  // settings: text inputs
  root.querySelectorAll('[data-set]').forEach(el=>{
    el.addEventListener('change', ()=>{
      Store.state.settings[el.dataset.set] = el.value;
      Store.notify();
    });
  });
  root.querySelectorAll('[data-set-num]').forEach(el=>{
    el.addEventListener('input', ()=>{
      Store.state.settings[el.dataset.setNum] = parseInt(el.value);
      Store.notify();
    });
  });
  root.querySelectorAll('[data-set-toggle]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const key = el.dataset.setToggle;
      Store.state.settings[key] = !Store.state.settings[key];
      Store.notify();
    });
  });

  // settings: theme segmented
  root.querySelectorAll('[data-theme-val]').forEach(el=>{
    el.addEventListener('click', ()=>{
      Store.state.settings.theme = el.dataset.themeVal;
      Store.setState({theme:el.dataset.themeVal});
    });
  });
  root.querySelectorAll('[data-density]').forEach(el=>{
    el.addEventListener('click', ()=>{
      Store.state.settings.density = el.dataset.density;
      Store.notify();
    });
  });

  // settings: violations
  root.querySelectorAll('[data-show-violations]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const fails = TASKS.filter(t=>t.parseFail);
      Store.showToast(`${fails.length} 个协议违规任务`, true);
    });
  });

  // settings: export
  root.querySelectorAll('[data-export]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const data = JSON.stringify({settings:Store.state.settings, projects:PROJECTS}, null, 2);
      const blob = new Blob([data], {type:'application/json'});
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url; a.download = 'aurapilot-config.json'; a.click();
      URL.revokeObjectURL(url);
      Store.showToast('配置已导出');
    });
  });

  // settings: reset
  root.querySelectorAll('[data-reset]').forEach(el=>{
    el.addEventListener('click', ()=>{
      Store.requestConfirm('确认恢复全部设置为默认值？', ()=>{
        Store.state.settings = {
          defaultDir:'/Users/gordon/dev', cliPath:'/usr/local/bin/aurapilot',
          autoStart:true, theme:'dark', fontSize:13, density:'standard',
          lenientMode:true, schemaStrictness:40, gitignoreByDefault:false
        };
        Store.closeConfirm();
        Store.showToast('设置已重置');
      });
    });
  });

  // settings: remove project
  root.querySelectorAll('[data-remove-project]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const name = el.dataset.removeProject;
      Store.requestConfirm(`确认移除项目 ${name}？（不会删除本地文件，仅从注册表移除）`, ()=>{
        delete PROJECTS[name];
        TASKS = TASKS.filter(t=>t.project!==name);
        Store.closeConfirm();
        Store.showToast(`已移除 ${name}`);
      });
    });
  });

  // confirm dialog
  root.querySelectorAll('[data-confirm-ok]').forEach(el=>{
    el.addEventListener('click', ()=>{
      const c = Store.state.confirmAction;
      if(c && c.onConfirm) c.onConfirm();
    });
  });
  root.querySelectorAll('[data-confirm-cancel]').forEach(el=>{
    el.addEventListener('click', ()=>Store.closeConfirm());
  });

  // segmented controls
  root.querySelectorAll('.seg, .seg-sm').forEach(g=>{
    g.addEventListener('click', e=>{
      const b = e.target.closest('button');
      if(!b) return;
      g.querySelectorAll('button').forEach(x=>x.classList.remove('on'));
      b.classList.add('on');
    });
  });

  // toggles
  root.querySelectorAll('.toggle').forEach(t=>{
    if(!t.dataset.setToggle && !t.id){
      t.addEventListener('click', ()=>t.classList.toggle('on'));
    }
  });
}

function closeModal(){
  const modalViews = ['detail','newtask','wizard'];
  if(modalViews.includes(Store.state.view)){
    setView(Store.state.lastBoardView || 'board');
  }
}

function saveDetail(){
  const id = Store.state.selectedTaskId;
  const t = Store.getTask(id);
  if(!t) return;
  const title = document.getElementById('detail-title')?.value.trim();
  const priority = document.getElementById('detail-priority')?.value;
  const type = document.getElementById('detail-type')?.value;
  const desc = document.getElementById('detail-desc')?.value;
  if(title){
    Store.updateTask(id, {title, priority, type, desc});
    Store.showToast('已保存 '+id);
    closeModal();
  }
}

// ============================================================
// 5. KEYBOARD SHORTCUTS
// ============================================================
document.addEventListener('keydown', e=>{
  const tag = e.target.tagName;
  const isInput = tag==='INPUT' || tag==='TEXTAREA' || tag==='SELECT';

  if(e.key==='Escape'){
    if(Store.state.confirmAction){ Store.closeConfirm(); return; }
    if(['detail','newtask','wizard'].includes(Store.state.view)){ closeModal(); return; }
  }

  if(isInput) return;

  if(e.key==='/'){
    e.preventDefault();
    const s = document.getElementById('searchInput');
    if(s){ s.focus(); }
  } else if(e.key==='n' || e.key==='N'){
    if(Store.state.view!=='newtask'){ e.preventDefault(); setView('newtask'); }
  } else if(e.key==='b' || e.key==='B'){
    if(Store.state.view==='board') setView('blocked');
    else if(Store.state.view==='blocked') setView('board');
  } else if(e.key==='e' || e.key==='E'){
    if(Store.state.selectedTaskId && Store.state.view==='board'){
      openTaskDetail(Store.state.selectedTaskId);
    }
  } else if(e.key==='d' || e.key==='D'){
    if(Store.state.selectedTaskId && Store.state.view!=='detail'){
      const id = Store.state.selectedTaskId;
      Store.requestConfirm(`确认删除任务 ${id}？此操作不可撤销。`, ()=>{
        Store.deleteTask(id);
        Store.closeConfirm();
        Store.showToast('已删除 '+id);
      });
    }
  } else if(e.key==='Enter'){
    if(Store.state.selectedTaskId && Store.state.view==='board'){
      openTaskDetail(Store.state.selectedTaskId);
    }
  } else if(['1','2','3','4'].includes(e.key)){
    if(Store.state.view==='board'){
      const states = ['backlog','in-progress','in-review','done'];
      const idx = parseInt(e.key)-1;
      const col = document.querySelector(`[data-col="${states[idx]}"] .col-head`);
      if(col) col.scrollIntoView({behavior:'smooth', block:'start'});
    }
  } else if(e.key==='j' || e.key==='J'){
    navigateTask(1);
  } else if(e.key==='k' || e.key==='K'){
    navigateTask(-1);
  }
});

function navigateTask(dir){
  const visible = document.querySelectorAll('[data-task]:not(.is-skeleton)');
  const tasks = [...visible];
  if(!tasks.length) return;
  let curIdx = tasks.findIndex(t=>t.dataset.task===Store.state.selectedTaskId);
  if(curIdx<0) curIdx = dir>0 ? -1 : 0;
  const nextIdx = Math.max(0, Math.min(tasks.length-1, curIdx+dir));
  const nextId = tasks[nextIdx].dataset.task;
  Store.setState({selectedTaskId:nextId});
  tasks[nextIdx].scrollIntoView({block:'nearest', behavior:'smooth'});
}

// ============================================================
// 6. INIT
// ============================================================
Store.subscribe(render);

// detect system theme preference
if(window.matchMedia){
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener('change', ()=>{ if(Store.state.theme==='system') applyTheme(); });
}

// start
render();
