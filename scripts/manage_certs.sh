#!/bin/bash
# scripts/manage_certs.sh

CERT_DIR="./certs"
mkdir -p "$CERT_DIR"

generate_ca() {
    echo "📜 Generating Certificate Authority..."
    openssl genrsa -out "$CERT_DIR/ca.key" 4096
    openssl req -x509 -new -nodes -key "$CERT_DIR/ca.key" -sha256 -days 3650 -out "$CERT_DIR/ca.crt" \
        -subj "/C=US/ST=State/L=City/O=VaultProject/CN=Vault-Root-CA"
}

issue_client_cert() {
    local name=$1
    echo "🔑 Issuing client certificate for: $name"

    # Generate private key
    openssl genrsa -out "$CERT_DIR/$name.key" 2048

    # Create CSR
    openssl req -new -key "$CERT_DIR/$name.key" -out "$CERT_DIR/$name.csr" \
        -subj "/C=US/ST=State/L=City/O=VaultProject/CN=$name"

    # Sign with CA
    openssl x509 -req -in "$CERT_DIR/$name.csr" -CA "$CERT_DIR/ca.crt" -CAkey "$CERT_DIR/ca.key" \
        -CAcreateserial -out "$CERT_DIR/$name.crt" -days 365 -sha256

    rm "$CERT_DIR/$name.csr"
    echo "✅ Success: $CERT_DIR/$name.crt created."
}

# Run logic
if [ ! -f "$CERT_DIR/ca.key" ]; then
    generate_ca
fi

issue_client_cert "admin_client"
