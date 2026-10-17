# Connexion RustHome via Authelia (OIDC)

RustHome délègue l'identification à Authelia (spec §8.2) : il ne stocke aucun mot de passe. Il ne gère que les **rôles**, à partir du nom d'utilisateur Authelia (`preferred_username`).

## 1. Déclarer le client dans Authelia

À faire par Hugo dans la configuration d'Authelia (rien n'est modifié automatiquement) :

```yaml
identity_providers:
  oidc:
    clients:
      - client_id: 'rusthome'
        client_name: 'RustHome'
        # Secret haché : générer avec
        #   authelia crypto hash generate pbkdf2 --variant sha512 --random --random.length 72
        # (la valeur en clair va dans RUSTHOME_OIDC_CLIENT_SECRET, le hash ici)
        client_secret: '$pbkdf2-sha512$...'
        public: false
        authorization_policy: 'two_factor'
        require_pkce: true
        pkce_challenge_method: 'S256'
        redirect_uris:
          - 'https://maison.example.org/api/v1/auth/callback'
        scopes: ['openid', 'profile']
        response_types: ['code']
        grant_types: ['authorization_code']
        token_endpoint_auth_method: 'client_secret_basic'
```

Remplacer `maison.example.org` par le nom public de RustHome derrière SWAG.

## 2. Configurer RustHome

```bash
RUSTHOME_AUTH=oidc
RUSTHOME_PUBLIC_URL=https://maison.example.org
RUSTHOME_OIDC_ISSUER=https://auth.example.org
RUSTHOME_OIDC_CLIENT_ID=rusthome
RUSTHOME_OIDC_CLIENT_SECRET=<secret en clair>
RUSTHOME_ROLES=admin=admin,hugo=resident,sokhna=resident
```

- Au démarrage, RustHome lit `https://auth.example.org/.well-known/openid-configuration`. S'il n'y arrive pas, il **refuse de démarrer** avec un message explicite.
- Un utilisateur authentifié par Authelia mais **absent de `RUSTHOME_ROLES` est refusé** (403).
- Le compte de Sokhna doit exister dans Authelia avec le nom `sokhna`.

## 3. Comportement

| Étape | URL |
|---|---|
| Connexion | `/api/v1/auth/login?next=/page` → redirection vers Authelia |
| Retour | `/api/v1/auth/callback` (code + PKCE, `state` et `nonce` vérifiés, usage unique) |
| Déconnexion | `/api/v1/auth/logout` (ferme la session RustHome ; la session Authelia reste ouverte) |

- Session : cookie `rusthome_session`, `HttpOnly`, `SameSite=Lax`, `Secure` si `RUSTHOME_PUBLIC_URL` est en HTTPS, durée 7 jours.
- Les sessions sont en mémoire : un redémarrage de RustHome demande de se reconnecter (passage par Authelia, souvent sans ressaisie grâce à sa propre session).
- Le parcours est testé de bout en bout avec un faux fournisseur (`crates/rusthome-api/tests/oidc_flow.rs`). **Le test contre le vrai Authelia reste à faire avec Hugo.**
