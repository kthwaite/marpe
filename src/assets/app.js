(function() {
    function encodePath(path) {
        return path.split('/').map(encodeURIComponent).join('/');
    }

    const currentPath = () => decodeURIComponent(location.pathname.replace(/^\/view\//, ''));

    const article = document.querySelector('.markdown-body');
    let renderQueue = Promise.resolve();

    function enqueueRender(render) {
        renderQueue = renderQueue.then(render).catch(console.error);
        return renderQueue;
    }

    async function renderMermaid(reset = false) {
        const diagrams = [...article.querySelectorAll('.mermaid')];
        for (const diagram of diagrams) {
            if (reset) {
                diagram.textContent = diagram.dataset.source;
                diagram.removeAttribute('data-processed');
            } else {
                diagram.dataset.source = diagram.textContent;
            }
        }
        if (diagrams.length) await mermaid.run({ nodes: diagrams });
    }

    async function renderEnhancements() {
        await MathJax.startup.promise;
        await Promise.all([MathJax.typesetPromise([article]), renderMermaid()]);
    }

    function replaceContent(html) {
        return enqueueRender(async () => {
            await MathJax.startup.promise;
            MathJax.typesetClear([article]);
            article.innerHTML = html;
            await renderEnhancements();
        });
    }

    // SSE
    const es = new EventSource('/events');
    es.onmessage = (e) => {
        const event = JSON.parse(e.data);
        if (event.type === 'FileChanged' && event.path === currentPath()) {
            fetch('/raw/' + encodePath(currentPath()))
                .then(r => r.text())
                .then(replaceContent);
        }
        if (event.type === 'FileAdded' || event.type === 'FileRemoved') {
            loadSidebar();
        }
    };

    // Sidebar
    async function loadSidebar() {
        const res = await fetch('/api/files');
        const files = await res.json();
        const tree = document.getElementById('file-tree');
        tree.innerHTML = '';
        files.forEach(f => {
            const li = document.createElement('li');
            const a = document.createElement('a');
            a.href = '/view/' + encodePath(f);
            a.textContent = f;
            a.onclick = (e) => {
                e.preventDefault();
                navigateTo(f);
            };
            if (f === currentPath()) a.classList.add('active');
            li.appendChild(a);
            tree.appendChild(li);
        });
    }

    async function renderPath(path) {
        const res = await fetch('/raw/' + encodePath(path));
        const html = await res.text();
        await replaceContent(html);
        document.querySelectorAll('#file-tree a').forEach(a => {
            a.classList.toggle('active', decodeURIComponent(a.pathname) === '/view/' + path);
        });
    }

    async function navigateTo(path) {
        await renderPath(path);
        history.pushState(null, '', '/view/' + encodePath(path));
    }

    window.onpopstate = () => {
        const path = currentPath();
        if (path) renderPath(path);
    };

    // Theme (Light/Dark)
    const themeToggle = document.getElementById('theme-toggle');
    function setTheme(theme, rerender = true) {
        document.body.classList.remove('theme-light', 'theme-dark');
        document.body.classList.add('theme-' + theme);
        themeToggle.textContent = 'Theme: ' + theme.charAt(0).toUpperCase() + theme.slice(1);
        localStorage.setItem('md-preview-theme', theme);
        mermaid.initialize({ startOnLoad: false, theme: theme === 'dark' ? 'dark' : 'default' });
        if (rerender) enqueueRender(() => renderMermaid(true));
    }
    themeToggle.onclick = () => {
        setTheme(document.body.classList.contains('theme-light') ? 'dark' : 'light');
    };
    const savedTheme = localStorage.getItem('md-preview-theme') || 'light';
    setTheme(savedTheme, false);

    // Style (GitHub/GitLab)
    const styleToggle = document.getElementById('style-toggle');
    function setStyle(style) {
        document.body.classList.remove('style-github', 'style-gitlab');
        document.body.classList.add('style-' + style);
        styleToggle.textContent = 'Style: ' + style.charAt(0).toUpperCase() + style.slice(1);
        localStorage.setItem('md-preview-style', style);
    }
    styleToggle.onclick = () => {
        setStyle(document.body.classList.contains('style-github') ? 'gitlab' : 'github');
    };
    const savedStyle = localStorage.getItem('md-preview-style') || 'github';
    setStyle(savedStyle);

    enqueueRender(renderEnhancements);
    loadSidebar();
})();
