#!/bin/bash

# Script to run end-to-end integration tests for MySQL MCP Server
# This script handles starting/stopping the MySQL test container

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
COMPOSE_FILE="docker-compose.test.yml"
MYSQL_HOST="${MYSQL_HOST:-localhost}"
MYSQL_PORT="${MYSQL_PORT:-3306}"
MYSQL_USER="${MYSQL_USER:-root}"
MYSQL_PASSWORD="${MYSQL_PASSWORD:-testpass}"
MAX_WAIT=60

echo -e "${GREEN}=== MySQL MCP Server E2E Tests ===${NC}"
echo ""

# Function to check if Docker is running
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        echo -e "${RED}Error: Docker is not running${NC}"
        echo "Please start Docker and try again"
        exit 1
    fi
}

# Function to check if docker-compose is available
check_docker_compose() {
    if ! command -v docker-compose &> /dev/null; then
        echo -e "${RED}Error: docker-compose is not installed${NC}"
        echo "Please install docker-compose and try again"
        exit 1
    fi
}

# Function to start MySQL container
start_mysql() {
    echo -e "${YELLOW}Starting MySQL test container...${NC}"
    docker-compose -f "$COMPOSE_FILE" up -d
    
    echo -e "${YELLOW}Waiting for MySQL to be ready...${NC}"
    local elapsed=0
    while [ $elapsed -lt $MAX_WAIT ]; do
        if docker-compose -f "$COMPOSE_FILE" exec -T mysql-test mysqladmin ping -h localhost -u root -p"$MYSQL_PASSWORD" --silent > /dev/null 2>&1; then
            echo -e "${GREEN}✓ MySQL is ready${NC}"
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
        echo -n "."
    done
    
    echo -e "${RED}Error: MySQL failed to start within ${MAX_WAIT} seconds${NC}"
    echo "Container logs:"
    docker-compose -f "$COMPOSE_FILE" logs
    return 1
}

# Function to stop MySQL container
stop_mysql() {
    echo -e "${YELLOW}Stopping MySQL test container...${NC}"
    docker-compose -f "$COMPOSE_FILE" down -v
    echo -e "${GREEN}✓ MySQL container stopped and cleaned up${NC}"
}

# Function to run tests
run_tests() {
    echo -e "${YELLOW}Running E2E integration tests...${NC}"
    echo ""
    
    MYSQL_HOST="$MYSQL_HOST" \
    MYSQL_PORT="$MYSQL_PORT" \
    MYSQL_USER="$MYSQL_USER" \
    MYSQL_PASSWORD="$MYSQL_PASSWORD" \
    cargo test --test e2e_integration_test -- --test-threads=1 --nocapture
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo ""
        echo -e "${GREEN}✓ All E2E tests passed!${NC}"
    else
        echo ""
        echo -e "${RED}✗ Some E2E tests failed${NC}"
    fi
    
    return $exit_code
}

# Cleanup function
cleanup() {
    local exit_code=$?
    echo ""
    if [ "$SKIP_CLEANUP" != "true" ]; then
        stop_mysql
    else
        echo -e "${YELLOW}Skipping cleanup (SKIP_CLEANUP=true)${NC}"
    fi
    exit $exit_code
}

# Main execution
main() {
    # Parse arguments
    local start_only=false
    local stop_only=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --start-only)
                start_only=true
                shift
                ;;
            --stop-only)
                stop_only=true
                shift
                ;;
            --skip-cleanup)
                export SKIP_CLEANUP=true
                shift
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --start-only     Only start the MySQL container"
                echo "  --stop-only      Only stop the MySQL container"
                echo "  --skip-cleanup   Don't stop the container after tests"
                echo "  --help           Show this help message"
                echo ""
                echo "Environment variables:"
                echo "  MYSQL_HOST       MySQL host (default: localhost)"
                echo "  MYSQL_PORT       MySQL port (default: 3306)"
                echo "  MYSQL_USER       MySQL user (default: root)"
                echo "  MYSQL_PASSWORD   MySQL password (default: testpass)"
                exit 0
                ;;
            *)
                echo -e "${RED}Unknown option: $1${NC}"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done
    
    # Check prerequisites
    check_docker
    check_docker_compose
    
    # Handle stop-only mode
    if [ "$stop_only" = true ]; then
        stop_mysql
        exit 0
    fi
    
    # Set up cleanup trap
    trap cleanup EXIT INT TERM
    
    # Start MySQL
    if ! start_mysql; then
        exit 1
    fi
    
    # Handle start-only mode
    if [ "$start_only" = true ]; then
        echo -e "${GREEN}MySQL container started. Use --stop-only to stop it.${NC}"
        trap - EXIT INT TERM  # Remove cleanup trap
        exit 0
    fi
    
    # Run tests
    run_tests
}

# Run main function
main "$@"
