#!/usr/bin/env bash
# Point one of fluxified's thunderforge Deployments at an image, and wait for
# Flux to roll it out.
#
#   scripts/flux-deploy.sh <file> <image-repo> <new-image> <deployment>
#   scripts/flux-deploy.sh app-deployment.yaml mbround18/thunderforgevtt \
#     mbround18/thunderforgevtt:sha-20183b5d thunderforge
#
# thunderforge-dev is Flux's (fluxified overlays/applications/thunderforge), so
# a deploy is a commit there: the `image:` line naming <image-repo> in <file>
# is rewritten, committed (signed, as every commit is) and pushed to main, and
# a rollback is a revert of that commit. The work happens in a fresh shallow
# clone, so no checkout of fluxified, and nothing uncommitted in one, is
# touched. A push that loses a race with another commit is retried on a fresh
# clone; nobody else's commit is ever rebased.
set -euo pipefail

file=$1 repo=$2 image=$3 deploy=$4
remote=${FLUXIFIED_REMOTE:-git@github.com:mbround18/fluxified.git}
overlay=overlays/applications/thunderforge
context=${KUBE_CONTEXT:-default}
namespace=${KUBE_NAMESPACE:-thunderforge-dev}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

commit=
for attempt in 1 2 3; do
  rm -rf "$work/fluxified"
  git clone --quiet --depth 1 --branch main "$remote" "$work/fluxified"
  cd "$work/fluxified"
  target="$overlay/$file"
  if ! grep -qE "^ +image: $repo[:@]" "$target"; then
    echo "flux-deploy: no 'image: $repo' line in $target" >&2
    exit 1
  fi
  sed -i -E "s#^( +image: )$repo[:@][^ ]*#\1$image#" "$target"
  if git diff --quiet; then
    echo "flux-deploy: $target already runs $image"
    commit=$(git rev-parse HEAD)
    break
  fi
  git add "$target"
  git commit --quiet -S -m "deploy(thunderforge): $deploy to ${image##*/}" \
    -m "From ThunderForgeVTT's make, at $(date -u +%Y-%m-%dT%H:%M:%SZ)."
  if git push --quiet origin HEAD:main; then
    commit=$(git rev-parse HEAD)
    break
  fi
  echo "flux-deploy: push lost a race (attempt $attempt), retrying on a fresh clone" >&2
  cd "$work"
done
[ -n "$commit" ] || { echo "flux-deploy: could not push to fluxified" >&2; exit 1; }
echo "flux-deploy: fluxified main is ${commit:0:7}"

stamp=$(date -Is)
kubectl --context "$context" -n flux-system annotate gitrepository roanoke-config \
  reconcile.fluxcd.io/requestedAt="$stamp" --overwrite >/dev/null
kubectl --context "$context" -n flux-system annotate kustomization roanoke \
  reconcile.fluxcd.io/requestedAt="$stamp" --overwrite >/dev/null

# The roanoke Kustomization is the whole cluster; give it ten minutes.
for _ in $(seq 1 120); do
  applied=$(kubectl --context "$context" -n flux-system get kustomization roanoke \
    -o jsonpath='{.status.lastAppliedRevision}')
  case $applied in *"$commit"*) break ;; esac
  sleep 5
done
case $applied in
  *"$commit"*) echo "flux-deploy: Flux applied ${commit:0:7}" ;;
  *) echo "flux-deploy: Flux has not applied ${commit:0:7} (at $applied)" >&2; exit 1 ;;
esac
kubectl --context "$context" -n "$namespace" rollout status "deploy/$deploy" --timeout=5m
