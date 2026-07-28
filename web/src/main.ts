import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { resolveTheme } from './theme'
import './style.css'

document.documentElement.dataset.theme = resolveTheme(localStorage.getItem('aurapilot-theme'))
createApp(App).use(createPinia()).mount('#app')
