import DefaultTheme from 'vitepress/theme';
import VitePressMermaid from '../components/VitePressMermaid.vue';

export default {
    extends: DefaultTheme,
    enhanceApp({app}) {
        app.component('VitePressMermaid', VitePressMermaid);
    },
};
