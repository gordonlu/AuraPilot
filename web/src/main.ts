import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { resolveWorldSkin, WORLD_SKIN_STORAGE_KEY } from './skins/worldSkin'
import { resolveTheme } from './theme'
import './style.css'

document.documentElement.dataset.theme = resolveTheme(localStorage.getItem('aurapilot-theme'))
document.documentElement.dataset.worldSkin = resolveWorldSkin(
  localStorage.getItem(WORLD_SKIN_STORAGE_KEY),
)
createApp(App).use(createPinia()).mount('#app')
