
async fn dashboard() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>APFSDS Dashboard</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 0; padding: 20px; background: #1a1b1e; color: #e0e0e0; }
        .container { max-width: 1000px; margin: 0 auto; }
        .card { background: #25262b; border-radius: 8px; padding: 20px; margin-bottom: 20px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); }
        h1, h2 { color: #fff; }
        .metric { font-size: 2em; font-weight: bold; color: #4dabf7; }
        table { width: 1000%; border-collapse: collapse; }
        th, td { text-align: left; padding: 12px; border-bottom: 1px solid #373a40; }
        th { color: #909296; }
        .status-ok { color: #40c057; }
    </style>
</head>
<body>
    <div class="container">
        <h1>APFSDS Control Plane</h1>
        
        <div class="card">
            <h2>System Status</h2>
            <div id="stats">Loading...</div>
        </div>

        <div class="card">
            <h2>Cluster Membership</h2>
            <div id="cluster">Loading...</div>
        </div>
    </div>

    <script>
        async fn refresh() {
            try {
                const res = await fetch('/admin/stats');
                const data = await res.json();
                document.getElementById('stats').innerHTML = `
                    <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px;">
                        <div>
                            <div class="metric">${data.active_connections}</div>
                            <div>Active Connections</div>
                        </div>
                         <div>
                            <div class="metric"><span class="status-ok">Active</span></div>
                            <div>System State</div>
                        </div>
                    </div>
                `;
            } catch (e) {
                console.error(e);
            }
        }
        refresh();
        setInterval(refresh, 5000);
    </script>
</body>
</html>
    "#)
}
