workflow "build-workflow" {
  on = "push"
  resolves = [
    "push docker images",
    "build x86_64 linux",
    "rbi/release-to-github",
  ]
}

action "build x86_64 linux" {
  uses = "actions/docker/cli@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  args = "build -t sirabien/rddns:dev ."
}

action "when on master" {
  uses = "actions/bin/filter@3c98a2679187369a2116d4f311568596d3725740"
  args = "branch master"
}

action "login to Docker Hub" {
  uses = "actions/docker/login@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  needs = ["when on master"]
  secrets = ["DOCKER_USERNAME", "DOCKER_PASSWORD"]
}

action "push docker images" {
  uses = "actions/docker/cli@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  args = "push sirabien/rddns:dev"
  needs = [
    "login to Docker Hub",
    "build x86_64 linux",
  ]
}

action "./.github/calculate-version" {
  uses = "./.github/calculate-version/"
  needs = ["when on master"]
}

action "rbi/release-to-github" {
  uses = "rbi/release-to-github@e8f88608207e7cbace447427d84a0a5c1520870d"
  needs = [
    "./.github/calculate-version",
    "build x86_64 linux"
  ]
  secrets = ["GITHUB_TOKEN"]
  args = "-x target/build-version"
}
