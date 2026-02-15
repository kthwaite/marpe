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

    // Theme
    const toggle = document.getElementById('theme-toggle');
    function setTheme(theme) {
        document.body.className = 'theme-' + theme;
        toggle.textContent = theme.charAt(0).toUpperCase() + theme.slice(1);
        localStorage.setItem('md-preview-theme', theme);
    }
    toggle.onclick = () => {
        setTheme(document.body.className === 'theme-github' ? 'gitlab' : 'github');
    };
    const saved = localStorage.getItem('md-preview-theme');
    if (saved) setTheme(saved);

    loadSidebar();
})();
