.PHONY: build up

build:
	docker build -t lemon-tokei .

up:
	docker run -d --name lemon-tokei -p 8000:8000 lemon-tokei