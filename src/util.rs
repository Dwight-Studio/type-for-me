/*
 *            ____           _       __    __     _____ __            ___
 *           / __ \_      __(_)___ _/ /_  / /_   / ___// /___  ______/ (_)___
 *          / / / / | /| / / / __ `/ __ \/ __/   \__ \/ __/ / / / __  / / __ \
 *         / /_/ /| |/ |/ / / /_/ / / / / /_    ___/ / /_/ /_/ / /_/ / / /_/ /
 *        /_____/ |__/|__/_/\__, /_/ /_/\__/   /____/\__/\__,_/\__,_/_/\____/
 *                         /____/
 *     Copyright (C) 2025 Dwight Studio
 *
 *     This program is free software: you can redistribute it and/or modify
 *     it under the terms of the GNU General Public License as published by
 *     the Free Software Foundation, either version 3 of the License, or
 *     (at your option) any later version.
 *
 *     This program is distributed in the hope that it will be useful,
 *     but WITHOUT ANY WARRANTY; without even the implied warranty of
 *     MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *     GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
#[macro_export]
macro_rules! logged_send {
    ( $sender:expr, $payload:expr ) => {
        if let Err(error) = $sender.send($payload) {
            error!("Error while sending command message: {error:?}");
        }
    };
}

#[macro_export]
macro_rules! handle_invalidation {
    ( $sender:expr, $session:ident, $msg:expr ) => {
        let mut stream = $session.receive_closed().await?;
        $sender.command(move |sender, shutdown| {
            shutdown
                .register(async move {
                    match stream.next().await {
                        Some(()) => {
                            debug!("Received shutdown signal?");
                            logged_send!(sender, $msg);
                        }
                        None => {
                            debug!("Received shutdown signal?");
                        }
                    }
                })
                .drop_on_shutdown()
        });
    };
}
