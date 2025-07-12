run:
	go run main.go

build:
	GOOS=windows GOARCH=amd64 go build -ldflags="-w -s" -o wuhu.exe main.go

build-linux:
	GOOS=linux GOARCH=amd64 go build -ldflags="-w -s" -o wuhu main.go

clean:
	rm -f wuhu.exe wuhu

.PHONY: run build build-linux clean 