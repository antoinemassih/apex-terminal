// ed-themes.jsx — Color schemes for the editorial trading app
// Each theme overrides the editorial CSS variable stack.
// Light themes preserve the "morning paper" metaphor; dark themes become "evening edition".

const ED_THEMES = {
  meridien: {
    label: 'Meridien · Salmon',
    isDark: false,
    swatch: ['#fff1e5', '#1c1a17', '#990f3d', '#2d7a3a', '#b5283a'],
    vars: {} // current default
  },

  nord: {
    label: 'Nord · Frost',
    isDark: false,
    swatch: ['#eceff4', '#2e3440', '#5e81ac', '#a3be8c', '#bf616a'],
    vars: {
      '--paper':    '#eceff4',
      '--paper-2':  '#e5e9f0',
      '--paper-3':  '#d8dee9',
      '--paper-4':  '#c8cdd6',
      '--ink-0':    '#2e3440',
      '--ink-1':    '#3b4252',
      '--ink-2':    '#4c566a',
      '--ink-3':    '#7b8294',
      '--ink-4':    '#a8adb8',
      '--rule-1':   '#d8dee9',
      '--rule-2':   '#9aa5b5',
      '--rule-3':   '#5e6779',
      '--claret':   '#5e81ac',
      '--navy':     '#2e3440',
      '--buy':      '#5b8a4d',
      '--sell':     '#a73d4a',
      '--buy-ink':  '#3d6435',
      '--sell-ink': '#7a2934',
    }
  },

  catppuccin: {
    label: 'Catppuccin · Mocha',
    isDark: true,
    swatch: ['#1e1e2e', '#cdd6f4', '#f5c2e7', '#a6e3a1', '#f38ba8'],
    vars: {
      '--paper':    '#1e1e2e',
      '--paper-2':  '#252537',
      '--paper-3':  '#2e2e44',
      '--paper-4':  '#393952',
      '--ink-0':    '#cdd6f4',
      '--ink-1':    '#bac2de',
      '--ink-2':    '#9399b2',
      '--ink-3':    '#7f849c',
      '--ink-4':    '#585b70',
      '--rule-1':   '#2e2e44',
      '--rule-2':   '#45456a',
      '--rule-3':   '#9399b2',
      '--claret':   '#f5c2e7',
      '--navy':     '#89b4fa',
      '--buy':      '#a6e3a1',
      '--sell':     '#f38ba8',
      '--buy-ink':  '#c8edc4',
      '--sell-ink': '#f7b3c5',
    }
  },

  rosepine: {
    label: 'Rose Pine · Main',
    isDark: true,
    swatch: ['#191724', '#e0def4', '#eb6f92', '#9ccfd8', '#eb6f92'],
    vars: {
      '--paper':    '#191724',
      '--paper-2':  '#1f1d2e',
      '--paper-3':  '#26233a',
      '--paper-4':  '#393552',
      '--ink-0':    '#e0def4',
      '--ink-1':    '#cdcbe6',
      '--ink-2':    '#908caa',
      '--ink-3':    '#6e6a86',
      '--ink-4':    '#403d52',
      '--rule-1':   '#26233a',
      '--rule-2':   '#393552',
      '--rule-3':   '#908caa',
      '--claret':   '#eb6f92',
      '--navy':     '#9ccfd8',
      '--buy':      '#9ccfd8',
      '--sell':     '#eb6f92',
      '--buy-ink':  '#c0e3e8',
      '--sell-ink': '#f4a3b9',
    }
  },

  solarized: {
    label: 'Solarized · Light',
    isDark: false,
    swatch: ['#fdf6e3', '#073642', '#cb4b16', '#859900', '#dc322f'],
    vars: {
      '--paper':    '#fdf6e3',
      '--paper-2':  '#f5eed4',
      '--paper-3':  '#eee8d5',
      '--paper-4':  '#d6cfb0',
      '--ink-0':    '#073642',
      '--ink-1':    '#586e75',
      '--ink-2':    '#657b83',
      '--ink-3':    '#93a1a1',
      '--ink-4':    '#c0bba6',
      '--rule-1':   '#e4dec3',
      '--rule-2':   '#a8a081',
      '--rule-3':   '#586e75',
      '--claret':   '#cb4b16',
      '--navy':     '#268bd2',
      '--buy':      '#859900',
      '--sell':     '#dc322f',
      '--buy-ink':  '#536100',
      '--sell-ink': '#8c1f1d',
    }
  },

  'solarized-dark': {
    label: 'Solarized · Dark',
    isDark: true,
    swatch: ['#002b36', '#eee8d5', '#b58900', '#859900', '#dc322f'],
    vars: {
      '--paper':    '#002b36',
      '--paper-2':  '#073642',
      '--paper-3':  '#0a3e4c',
      '--paper-4':  '#114a59',
      '--ink-0':    '#fdf6e3',
      '--ink-1':    '#eee8d5',
      '--ink-2':    '#93a1a1',
      '--ink-3':    '#657b83',
      '--ink-4':    '#3a525a',
      '--rule-1':   '#114a59',
      '--rule-2':   '#2a6e85',
      '--rule-3':   '#93a1a1',
      '--claret':   '#b58900',
      '--navy':     '#268bd2',
      '--buy':      '#a3c000',
      '--sell':     '#dc322f',
      '--buy-ink':  '#cfe084',
      '--sell-ink': '#f08785',
    }
  },

  gruvbox: {
    label: 'Gruvbox · Dark',
    isDark: true,
    swatch: ['#282828', '#ebdbb2', '#fe8019', '#b8bb26', '#fb4934'],
    vars: {
      '--paper':    '#282828',
      '--paper-2':  '#32302f',
      '--paper-3':  '#3c3836',
      '--paper-4':  '#504945',
      '--ink-0':    '#fbf1c7',
      '--ink-1':    '#ebdbb2',
      '--ink-2':    '#d5c4a1',
      '--ink-3':    '#a89984',
      '--ink-4':    '#665c54',
      '--rule-1':   '#3c3836',
      '--rule-2':   '#665c54',
      '--rule-3':   '#a89984',
      '--claret':   '#fe8019',
      '--navy':     '#83a598',
      '--buy':      '#b8bb26',
      '--sell':     '#fb4934',
      '--buy-ink':  '#d6db78',
      '--sell-ink': '#fb928a',
    }
  },

  dracula: {
    label: 'Dracula',
    isDark: true,
    swatch: ['#282a36', '#f8f8f2', '#ff79c6', '#50fa7b', '#ff5555'],
    vars: {
      '--paper':    '#282a36',
      '--paper-2':  '#2f3140',
      '--paper-3':  '#383a4a',
      '--paper-4':  '#44475a',
      '--ink-0':    '#f8f8f2',
      '--ink-1':    '#d8d8d0',
      '--ink-2':    '#9a9ab0',
      '--ink-3':    '#6c6e85',
      '--ink-4':    '#44475a',
      '--rule-1':   '#383a4a',
      '--rule-2':   '#5b5e76',
      '--rule-3':   '#9a9ab0',
      '--claret':   '#ff79c6',
      '--navy':     '#bd93f9',
      '--buy':      '#50fa7b',
      '--sell':     '#ff5555',
      '--buy-ink':  '#9affb4',
      '--sell-ink': '#ff9999',
    }
  },

  kanagawa: {
    label: 'Kanagawa · Wave',
    isDark: true,
    swatch: ['#1f1f28', '#dcd7ba', '#ffa066', '#76946a', '#e46876'],
    vars: {
      '--paper':    '#1f1f28',
      '--paper-2':  '#2a2a37',
      '--paper-3':  '#363646',
      '--paper-4':  '#54546d',
      '--ink-0':    '#dcd7ba',
      '--ink-1':    '#c8c093',
      '--ink-2':    '#a6a08c',
      '--ink-3':    '#727169',
      '--ink-4':    '#54546d',
      '--rule-1':   '#363646',
      '--rule-2':   '#54546d',
      '--rule-3':   '#a6a08c',
      '--claret':   '#ffa066',
      '--navy':     '#7e9cd8',
      '--buy':      '#76946a',
      '--sell':     '#e46876',
      '--buy-ink':  '#a8c499',
      '--sell-ink': '#f0a3ad',
    }
  },

  'kanagawa-light': {
    label: 'Kanagawa · Lotus',
    isDark: false,
    swatch: ['#f2ecbc', '#43436c', '#cc6d00', '#6f894e', '#c84053'],
    vars: {
      '--paper':    '#f2ecbc',
      '--paper-2':  '#e9e2ab',
      '--paper-3':  '#dcd5a2',
      '--paper-4':  '#c9c294',
      '--ink-0':    '#43436c',
      '--ink-1':    '#5a5a86',
      '--ink-2':    '#6c6c95',
      '--ink-3':    '#8a8a9e',
      '--ink-4':    '#b4afa1',
      '--rule-1':   '#dcd5a2',
      '--rule-2':   '#b4afa1',
      '--rule-3':   '#6c6c95',
      '--claret':   '#cc6d00',
      '--navy':     '#4d699b',
      '--buy':      '#6f894e',
      '--sell':     '#c84053',
      '--buy-ink':  '#4a5d33',
      '--sell-ink': '#892b39',
    }
  },

  everforest: {
    label: 'Everforest · Hard',
    isDark: true,
    swatch: ['#272e33', '#d3c6aa', '#e69875', '#a7c080', '#e67e80'],
    vars: {
      '--paper':    '#272e33',
      '--paper-2':  '#2d353b',
      '--paper-3':  '#343f44',
      '--paper-4':  '#3d484d',
      '--ink-0':    '#d3c6aa',
      '--ink-1':    '#bfb699',
      '--ink-2':    '#9da9a0',
      '--ink-3':    '#7a8478',
      '--ink-4':    '#4f585e',
      '--rule-1':   '#343f44',
      '--rule-2':   '#475258',
      '--rule-3':   '#9da9a0',
      '--claret':   '#e69875',
      '--navy':     '#7fbbb3',
      '--buy':      '#a7c080',
      '--sell':     '#e67e80',
      '--buy-ink':  '#cdd9b3',
      '--sell-ink': '#f2b2b3',
    }
  },

  paper: {
    label: 'Paper · Ivory',
    isDark: false,
    swatch: ['#f4ecd8', '#1a1a1a', '#a8341c', '#3a6b2a', '#b03a2e'],
    vars: {
      '--paper':    '#f4ecd8',
      '--paper-2':  '#ece3c8',
      '--paper-3':  '#e0d5b4',
      '--paper-4':  '#c8bd9a',
      '--ink-0':    '#1a1a1a',
      '--ink-1':    '#3a3a36',
      '--ink-2':    '#6a6a60',
      '--ink-3':    '#8e8a7c',
      '--ink-4':    '#b0ac9c',
      '--rule-1':   '#d4c9a8',
      '--rule-2':   '#a89e7e',
      '--rule-3':   '#5a5448',
      '--claret':   '#a8341c',
      '--navy':     '#1c3a6b',
      '--buy':      '#3a6b2a',
      '--sell':     '#b03a2e',
      '--buy-ink':  '#23461a',
      '--sell-ink': '#7a2620',
    }
  },

  newsprint: {
    label: 'Newsprint · Cream',
    isDark: false,
    swatch: ['#fbf6e9', '#222', '#1e6091', '#1f7a3a', '#a02323'],
    vars: {
      '--paper':    '#fbf6e9',
      '--paper-2':  '#f4eddb',
      '--paper-3':  '#ebe3c8',
      '--paper-4':  '#d8cfaf',
      '--ink-0':    '#222222',
      '--ink-1':    '#3a3a3a',
      '--ink-2':    '#6a6a60',
      '--ink-3':    '#8e8a7c',
      '--ink-4':    '#b8b29c',
      '--rule-1':   '#dcd4b8',
      '--rule-2':   '#b8b08e',
      '--rule-3':   '#6a6452',
      '--claret':   '#1e6091',
      '--navy':     '#0d1d33',
      '--buy':      '#1f7a3a',
      '--sell':     '#a02323',
      '--buy-ink':  '#114d22',
      '--sell-ink': '#6e1818',
    }
  },
};

function applyEdTheme(name){
  const t = ED_THEMES[name] || ED_THEMES.meridien;
  const root = document.documentElement;

  root.classList.toggle('ed-dark', !!t.isDark);
  root.setAttribute('data-ed-theme', name);

  // Wipe all previously-set vars from any theme so switching is clean
  const allKeys = new Set();
  Object.values(ED_THEMES).forEach(th => Object.keys(th.vars).forEach(k => allKeys.add(k)));
  allKeys.forEach(k => root.style.removeProperty(k));

  // Apply this theme's overrides
  Object.entries(t.vars).forEach(([k, v]) => root.style.setProperty(k, v));
}

window.ED_THEMES = ED_THEMES;
window.applyEdTheme = applyEdTheme;
