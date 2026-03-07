#!/bin/bash
set -e

echo "🔍 Starting System Health Check..."

# Check Container Status
echo "🐳 Checking Container Status..."
podman ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Check Postgres
echo "🐘 Checking Database Connection..."
if podman exec vipertrade-postgres pg_isready -U viper; then
    echo "✅ Database is ready"
else
    echo "❌ Database is NOT ready"
    exit 1
fi

# Check API
echo "🔌 Checking API Endpoint..."
if curl -s http://localhost:8080/ > /dev/null; then
    echo "✅ API is reachable"
else
    echo "❌ API is NOT reachable"
    exit 1
fi

# Check Web
echo "🌐 Checking Web Interface..."
if curl -s http://localhost:3000/ > /dev/null; then
    echo "✅ Web UI is reachable"
else
    echo "❌ Web UI is NOT reachable"
    exit 1
fi

echo "✅ Health Check Passed!"
