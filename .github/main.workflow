workflow "build-workflow" {
  on = "push"
  resolves = [
    "push docker images",
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
  needs = ["build x86_64 linux"]
}

action "login to Docker Hub" {
  uses = "actions/docker/login@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  needs = ["when on master"]
  secrets = ["DOCKER_USERNAME", "DOCKER_PASSWORD"]
}

action "push docker images" {
  uses = "actions/docker/cli@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  args = "push sirabien/rddns:dev"
  needs = ["login to Docker Hub"]
}

action "calculate version" {
  uses = "./.github/calculate-version/"
  needs = ["when on master"]
}

action "extract binary" {
  uses = "./.github/extract-binary/"
  needs = ["when on master"]
}

action "rbi/release-to-github" {
  uses = "rbi/release-to-github@04e7321e80a5ed9150686c2fc9e03feab2c2b26f"
  needs = [
    "calculate version",
    "extract binary"
  ]
  secrets = ["GITHUB_TOKEN"]
  args = "-x target/build-version -f target/rddns:rddns-x86_64-linux"
}
