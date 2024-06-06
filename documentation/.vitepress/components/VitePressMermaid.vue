<script setup lang="ts">
import {useData} from 'vitepress';
import mermaid from 'mermaid';
import {computed, nextTick, onMounted, ref, watch} from 'vue';

const props = defineProps<{
    code: string;
}>();

const {isDark} = useData();
const container = ref<HTMLElement | null>(null);
const theme = computed(() => (isDark.value ? 'dark' : 'default'));

function decodeHtml(value: string): string {
    return value
        .replaceAll('&lt;', '<')
        .replaceAll('&gt;', '>')
        .replaceAll('&amp;', '&')
        .replaceAll('&quot;', '"')
        .replaceAll('&#39;', "'");
}

async function renderDiagram() {
    const element = container.value;
    if (!element) {
        return;
    }

    element.textContent = decodeHtml(props.code);
    element.removeAttribute('data-processed');

    mermaid.initialize({
        startOnLoad: false,
        theme: theme.value,
        securityLevel: 'loose',
    });

    await mermaid.run({nodes: [element]});
}

watch(theme, async () => {
    await nextTick();
    await renderDiagram();
});

onMounted(async () => {
    await nextTick();
    await renderDiagram();
});
</script>

<template>
    <div class="mermaid-diagram">
        <pre ref="container" class="mermaid" />
    </div>
</template>

<style scoped>
.mermaid-diagram {
    margin: 1rem 0;
    overflow-x: auto;
}

.mermaid-diagram :deep(svg) {
    display: block;
    margin: 0 auto;
    max-width: 100%;
}
</style>
