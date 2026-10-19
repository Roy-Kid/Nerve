export type Locale = 'en' | 'zh';

export const locales: { id: Locale; label: string; short: string }[] = [
  { id: 'en', label: 'English', short: 'EN' },
  { id: 'zh', label: '中文', short: '中' },
];

type NavCopy = {
  home: string;
  macos: string;
  tmux: string;
  vscode: string;
  docs: string;
  get: string;
  github: string;
  openMenu: string;
  closeMenu: string;
  productNavigation: string;
  mobileNavigation: string;
  footerNavigation: string;
  switchLanguage: string;
};

type HubSurfaceCopy = {
  kicker: string;
  title: string;
  body: string;
  cta: string;
};

type HubCopy = {
  brand: string;
  kicker: string;
  tagline: string;
  product: {
    kicker: string;
    title: [string, string];
    body: string;
    facts: { title: string; body: string }[];
    live: string;
  };
  macos: HubSurfaceCopy;
  tmux: HubSurfaceCopy;
  vscode: HubSurfaceCopy;
  open: {
    kicker: string;
    title: [string, string];
    body: string;
    sourceCta: string;
    protocolCta: string;
    producers: string;
    producerRole: string;
    hub: string;
    surfaces: string;
    surfaceRole: string;
  };
};

type FooterCopy = {
  download: string;
  tagline: string;
};

type MacosCopy = {
  headline: [string, string];
  subhead: string;
  openDocs: string;
  ribbonPreviewLabel: string;
  storyTitle: string;
  truths: { title: string; body: string }[];
  getTitle: string;
};

type VscodeCopy = {
  headline: [string, string];
  subhead: string;
  openDocs: string;
  previewLabel: string;
  truths: { title: string; body: string }[];
};

type DocsUiCopy = {
  sidebarLabel: string;
  protocol: string;
  overview: string;
  eyebrow: string;
  indexEyebrow: string;
  indexTitle: string;
  indexLede: string;
  pagerLabel: string;
  previous: string;
  next: string;
};

type PreviewJobCopy = {
  name: string;
  detail: string;
  time: string;
};

type PreviewCopy = {
  status: Record<
    'problem' | 'attention' | 'running' | 'monitor' | 'success' | 'inactive',
    string
  >;
  mac: {
    livePreview: string;
    terminalReady: string;
    finder: string;
    file: string;
    edit: string;
    view: string;
    ribbonLabel: string;
    panelLabel: string;
    countsLabel: string;
    jobs: PreviewJobCopy[];
  };
  tmux: {
    label: string;
    filter: string;
    jobs: PreviewJobCopy[];
    activity: string;
    reviewRequested: string;
    checkoutReady: string;
    implementationDone: string;
    testsPassed: string;
    waitingForReview: string;
    prompt: string;
  };
  vscode: {
    label: string;
    sidebar: string;
    editor: string;
    status: string;
    host: string;
    jobs: PreviewJobCopy[];
  };
};

export type Messages = {
  nav: NavCopy;
  hub: HubCopy;
  footer: FooterCopy;
  macos: MacosCopy;
  vscode: VscodeCopy;
  docsUi: DocsUiCopy;
  preview: PreviewCopy;
};

export const messages: Record<Locale, Messages> = {
  en: {
    nav: {
      home: 'Home',
      macos: 'macOS',
      tmux: 'tmux',
      vscode: 'VS Code',
      docs: 'Docs',
      get: 'Install',
      github: 'GitHub',
      openMenu: 'Open navigation',
      closeMenu: 'Close navigation',
      productNavigation: 'Product',
      mobileNavigation: 'Product mobile',
      footerNavigation: 'Footer',
      switchLanguage: 'Switch language',
    },
    hub: {
      brand: 'Nerve',
      kicker: 'Your machines, connected by an open nervous system.',
      tagline: 'Job monitor for long-running work across machines.',
      product: {
        kicker: 'The product',
        title: ['One job list.', 'Across every machine.'],
        body:
          'Nerve gathers live jobs from this Mac and remote machines into one local view. See what is running, waiting for you, broken, finished, or quiet—without checking every window.',
        facts: [
          {
            title: 'Structured status',
            body: 'Lifecycle events decide status. Nerve never guesses from conversation text.',
          },
          {
            title: 'Every machine',
            body: 'Loopback ingest and SSH tunnels bring local and remote work into the same list.',
          },
          {
            title: 'Memory only',
            body: 'Live jobs stay in RAM and disappear when the local hub exits.',
          },
        ],
        live: 'Live from 3 machines',
      },
      macos: {
        kicker: 'macOS',
        title: 'In the menu bar.',
        body: 'Every job, always in sight.',
        cta: 'View macOS',
      },
      tmux: {
        kicker: 'tmux',
        title: 'In the terminal.',
        body: 'The same live view, inside tmux.',
        cta: 'View tmux',
      },
      vscode: {
        kicker: 'VS Code',
        title: 'In the editor.',
        body: 'The same live view, where the agent already is.',
        cta: 'View VS Code',
      },
      open: {
        kicker: 'Open source · Open protocol',
        title: ['Bring any producer.', 'Build any surface.'],
        body:
          'Nerve is not limited to AI agents. Send snapshots from a build, test runner, service, or your own tool; consume the same stream in a new surface, or implement your own backend. The source and wire contract are open.',
        sourceCta: 'View source',
        protocolCta: 'Read the protocol',
        producers: 'Agents · builds · tests · your tool',
        producerRole: 'Pushes full job snapshots',
        hub: 'Local state authority',
        surfaces: 'Surfaces · dashboards · your backend',
        surfaceRole: 'Reads full-state frames',
      },
    },
    footer: {
      download: 'Install',
      tagline: 'Open source. Local first.',
    },
    macos: {
      headline: ['The whole queue.', 'In the menu bar.'],
      subhead:
        'The ribbon compresses every live job into one glance. Open it for the job, machine, status, detail, and elapsed time.',
      openDocs: 'Set up macOS',
      ribbonPreviewLabel: 'Live ribbon preview',
      storyTitle: 'Status without another window.',
      truths: [
        { title: 'Always visible', body: 'Six states stay distinct in one compact ribbon.' },
        {
          title: 'Details on demand',
          body: 'Open the panel for jobs grouped by machine.',
        },
        {
          title: 'Return in one click',
          body: 'Open or focus the terminal where the job is running.',
        },
      ],
      getTitle: 'Everything else is in the docs.',
    },
    vscode: {
      headline: ['The same jobs.', 'Inside VS Code.'],
      subhead:
        'The Activity Bar holds the full queue while the status bar makes attention impossible to miss. Focus jumps straight to the matching terminal.',
      openDocs: 'Open VS Code docs',
      previewLabel: 'VS Code jobs preview',
      truths: [
        {
          title: 'Full job tree',
          body: 'Browse every machine and job without leaving the editor.',
        },
        {
          title: 'Native focus',
          body: 'Jump to this window’s terminal with one command.',
        },
        {
          title: 'Shared live state',
          body: 'The same full-state stream read by macOS and tmux.',
        },
      ],
    },
    docsUi: {
      sidebarLabel: 'Documentation',
      protocol: 'Protocol',
      overview: 'Overview',
      eyebrow: 'Docs',
      indexEyebrow: 'Open protocol',
      indexTitle: 'The whole protocol, written down.',
      indexLede:
        'Install the app, wire agent plugins, push snapshots, and read the exact contract every client reads. Surfaces live on the home page; this handbook keeps the implementation details in one place.',
      pagerLabel: 'Adjacent docs',
      previous: 'Previous',
      next: 'Next',
    },
    preview: {
      status: {
        problem: 'Problem',
        attention: 'Needs you',
        running: 'Running',
        monitor: 'Monitor',
        success: 'Completed',
        inactive: 'Ready / idle',
      },
      mac: {
        livePreview: 'Live preview',
        terminalReady: 'ready · built in 0.10 s',
        finder: 'Finder',
        file: 'File',
        edit: 'Edit',
        view: 'View',
        ribbonLabel: 'Nerve status ribbon',
        panelLabel: 'Nerve status panel preview',
        countsLabel: 'Status counts',
        jobs: [
          { name: 'Deploy preview', detail: 'Health check failed', time: '8 min' },
          { name: 'Unit tests', detail: 'Approval required', time: '1 min' },
          { name: 'Build archive', detail: 'Finished cleanly', time: '2 min' },
          { name: 'Claude Code', detail: 'Editing website', time: '18 sec' },
          { name: 'Build log', detail: 'Watching stream', time: '3 min' },
          { name: 'Local watcher', detail: 'No recent signal', time: '12 min' },
        ],
      },
      tmux: {
        label: 'tmux sidebar preview',
        filter: 'all',
        jobs: [
          { name: 'deploy', detail: 'Deploy failed', time: '4m' },
          { name: 'nerve', detail: 'Needs your review', time: 'now' },
          { name: 'ios-build', detail: 'Tests passed', time: '1m' },
          { name: 'checkout', detail: 'Implementing auth', time: '3m' },
          { name: 'logs', detail: 'Watching stream', time: '2m' },
          { name: 'docs', detail: 'Ready', time: '12m' },
        ],
        activity: 'Activity',
        reviewRequested: 'Review requested',
        checkoutReady: 'Checkout flow ready',
        implementationDone: 'agent finished the implementation',
        testsPassed: 'tests passed in 42 seconds',
        waitingForReview: 'waiting for your review',
        prompt: 'Review the change and ship it.',
      },
      vscode: {
        label: 'VS Code jobs preview',
        sidebar: 'Jobs',
        editor: 'claude-code · nerve',
        status: 'Nerve 1↑',
        host: 'nerve',
        jobs: [
          { name: 'Deploy preview', detail: 'Health check failed', time: '8 min' },
          { name: 'Unit tests', detail: 'Approval required', time: '1 min' },
          { name: 'Build archive', detail: 'Finished cleanly', time: '2 min' },
          { name: 'Claude Code', detail: 'Editing website', time: '18 sec' },
          { name: 'Build log', detail: 'Watching stream', time: '3 min' },
          { name: 'Local watcher', detail: 'No recent signal', time: '12 min' },
        ],
      },
    },
  },
  zh: {
    nav: {
      home: '首页',
      macos: 'macOS',
      tmux: 'tmux',
      vscode: 'VS Code',
      docs: '文档',
      get: '安装',
      github: 'GitHub',
      openMenu: '打开导航',
      closeMenu: '关闭导航',
      productNavigation: '产品',
      mobileNavigation: '移动端产品导航',
      footerNavigation: '页脚',
      switchLanguage: '切换语言',
    },
    hub: {
      brand: 'Nerve',
      kicker: '让你的每台机器，共用一套开放神经系统。',
      tagline: '跨机器长时任务监控器。',
      product: {
        kicker: '产品详情',
        title: ['一个任务列表。', '覆盖每台机器。'],
        body:
          'Nerve 把本机和远端机器上的实时任务汇入一个本地视图。运行中、需要你、出错、完成或静默，不用再逐个窗口检查。',
        facts: [
          {
            title: '结构化状态',
            body: '状态只由生命周期事件决定，绝不猜测对话文本。',
          },
          {
            title: '覆盖每台机器',
            body: '固定回环接入配合 SSH 隧道，本地与远端任务进入同一列表。',
          },
          {
            title: '只在内存中',
            body: '实时任务只存于内存，本地 hub 退出后即清空。',
          },
        ],
        live: '来自 3 台机器的实时状态',
      },
      macos: {
        kicker: 'macOS',
        title: '常驻菜单栏。',
        body: '所有任务，始终在眼前。',
        cta: '看 macOS',
      },
      tmux: {
        kicker: 'tmux',
        title: '就在终端里。',
        body: '同一个实时视图，也在 tmux。',
        cta: '看 tmux',
      },
      vscode: {
        kicker: 'VS Code',
        title: '就在编辑器里。',
        body: '同一个实时视图，agent 已经在的地方。',
        cta: '看 VS Code',
      },
      open: {
        kicker: '开放源码 · 开放协议',
        title: ['接入任何发送端。', '构建任何显示面。'],
        body:
          'Nerve 不局限于 AI agent。构建、测试、服务或你自己的工具都能发送快照；你也可以消费同一条状态流，增加新的 surface，甚至实现自己的后端。源码和通讯协议全部开放。',
        sourceCta: '查看源码',
        protocolCta: '阅读协议',
        producers: 'Agent · 构建 · 测试 · 你的工具',
        producerRole: '发送完整任务快照',
        hub: '本地状态中心',
        surfaces: 'Surface · 仪表盘 · 你的后端',
        surfaceRole: '读取完整状态帧',
      },
    },
    footer: {
      download: '安装',
      tagline: '开源，本地优先。',
    },
    macos: {
      headline: ['完整任务队列。', '就在菜单栏。'],
      subhead: '状态条把所有实时任务压缩成一眼；展开后可查看任务、机器、状态、详情和已用时间。',
      openDocs: '配置 macOS',
      ribbonPreviewLabel: '实时状态条预览',
      storyTitle: '状态清楚，不用再开窗口。',
      truths: [
        { title: '始终可见', body: '六种状态在一条紧凑状态条里清楚区分。' },
        { title: '需要时看详情', body: '展开面板，按机器查看每个任务。' },
        { title: '一点就能返回', body: '直接打开或聚焦任务所在的终端。' },
      ],
      getTitle: '其他细节，都在文档里。',
    },
    vscode: {
      headline: ['同一批任务。', '就在 VS Code。'],
      subhead: 'Activity Bar 展示完整队列，状态栏让待处理任务无法错过；Focus 会直接跳到对应终端。',
      openDocs: '打开 VS Code 文档',
      previewLabel: 'VS Code 任务预览',
      truths: [
        {
          title: '完整任务树',
          body: '不离开编辑器，查看每台机器和每个任务。',
        },
        {
          title: '原生聚焦',
          body: '一条命令，跳到当前窗口内对应的终端。',
        },
        {
          title: '共享实时状态',
          body: '与 macOS、tmux 读取同一条完整状态流。',
        },
      ],
    },
    docsUi: {
      sidebarLabel: '文档',
      protocol: '协议',
      overview: '概览',
      eyebrow: '文档',
      indexEyebrow: '开放协议',
      indexTitle: '整套协议，全部写清。',
      indexLede:
        '安装应用、接入 agent 插件、推送快照，再查看所有客户端共用的准确协议。显示面在首页；这本手册把实现细节集中在一处。',
      pagerLabel: '相邻文档',
      previous: '上一篇',
      next: '下一篇',
    },
    preview: {
      status: {
        problem: '出问题',
        attention: '需要你',
        running: '运行中',
        monitor: '监视中',
        success: '已完成',
        inactive: '就绪 / 空闲',
      },
      mac: {
        livePreview: '实时预览',
        terminalReady: '就绪 · 0.10 秒构建完成',
        finder: '访达',
        file: '文件',
        edit: '编辑',
        view: '显示',
        ribbonLabel: 'Nerve 状态条',
        panelLabel: 'Nerve 状态面板预览',
        countsLabel: '状态计数',
        jobs: [
          { name: '预览部署', detail: '健康检查失败', time: '8 分钟' },
          { name: '单元测试', detail: '需要批准', time: '1 分钟' },
          { name: '归档构建', detail: '顺利完成', time: '2 分钟' },
          { name: 'Claude Code', detail: '正在编辑网站', time: '18 秒' },
          { name: '构建日志', detail: '正在监视输出', time: '3 分钟' },
          { name: '本地监视器', detail: '最近没有信号', time: '12 分钟' },
        ],
      },
      tmux: {
        label: 'tmux 侧边栏预览',
        filter: '全部',
        jobs: [
          { name: 'deploy', detail: '部署失败', time: '4m' },
          { name: 'nerve', detail: '等你审查', time: '现在' },
          { name: 'ios-build', detail: '测试通过', time: '1m' },
          { name: 'checkout', detail: '正在实现认证', time: '3m' },
          { name: 'logs', detail: '正在监视输出', time: '2m' },
          { name: 'docs', detail: '就绪', time: '12m' },
        ],
        activity: '活动',
        reviewRequested: '已请求审查',
        checkoutReady: '结账流程已就绪',
        implementationDone: 'agent 已完成实现',
        testsPassed: '测试已在 42 秒内通过',
        waitingForReview: '正在等待你审查',
        prompt: '审查这次改动并发布。',
      },
      vscode: {
        label: 'VS Code 任务预览',
        sidebar: '任务',
        editor: 'claude-code · nerve',
        status: 'Nerve 1↑',
        host: 'nerve',
        jobs: [
          { name: '预览部署', detail: '健康检查失败', time: '8 分钟' },
          { name: '单元测试', detail: '需要批准', time: '1 分钟' },
          { name: '归档构建', detail: '顺利完成', time: '2 分钟' },
          { name: 'Claude Code', detail: '正在编辑网站', time: '18 秒' },
          { name: '构建日志', detail: '正在监视输出', time: '3 分钟' },
          { name: '本地监视器', detail: '最近没有信号', time: '12 分钟' },
        ],
      },
    },
  },
};
