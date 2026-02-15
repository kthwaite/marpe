(function() {
    const currentPath = () => decodeURIComponent(location.pathname.replace(/^\/view\//, ''));

    // SSE
    const es = new EventSource('/events');
    es.onmessage = (e) => {
        const event = JSON.parse(e.data);
        if (event.type === 'FileChanged' && event.path === currentPath()) {
            fetch('/raw/' + currentPath())
                .then(r => r.text())
                .then(html => { document.querySelector('.markdown-body').innerHTML = html; });
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
            a.href = '/view/' + f;
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

    async function navigateTo(path) {
        const res = await fetch('/raw/' + path);
        const html = await res.text();
        document.querySelector('.markdown-body').innerHTML = html;
        history.pushState(null, '', '/view/' + path);
        document.querySelectorAll('#file-tree a').forEach(a => {
            a.classList.toggle('active', a.href.endsWith('/view/' + path));
        });
    }

    window.onpopstate = () => {
        const path = currentPath();
        if (path) navigateTo(path);
    };

    // Theme (Light/Dark)
    const themeToggle = document.getElementById('theme-toggle');
    function setTheme(theme) {
        document.body.classList.remove('theme-light', 'theme-dark');
        document.body.classList.add('theme-' + theme);
        themeToggle.textContent = theme.charAt(0).toUpperCase() + theme.slice(1);
        localStorage.setItem('md-preview-theme', theme);
    }
    themeToggle.onclick = () => {
        setTheme(document.body.classList.contains('theme-light') ? 'dark' : 'light');
    };
    const savedTheme = localStorage.getItem('md-preview-theme') || 'light';
    setTheme(savedTheme);

    // Style (GitHub/GitLab)
    const styleToggle = document.getElementById('style-toggle');
    function setStyle(style) {
        document.body.classList.remove('style-github', 'style-gitlab');
        document.body.classList.add('style-' + style);
        styleToggle.textContent = style.charAt(0).toUpperCase() + style.slice(1);
        localStorage.setItem('md-preview-style', style);
    }
    styleToggle.onclick = () => {
        setStyle(document.body.classList.contains('style-github') ? 'gitlab' : 'github');
    };
    const savedStyle = localStorage.getItem('md-preview-style') || 'github';
    setStyle(savedStyle);

    loadSidebar();
})();
